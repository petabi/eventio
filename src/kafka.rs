//! Reading/writing events from/to Apache Kafka servers.

use crate::fluentd::{Entry, ForwardMode};
use crate::Error;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::producer::{Producer, Record, RequiredAcks};
use rmp_serde::Serializer;
use serde::Serialize;
use std::convert::TryInto;

#[derive(Debug)]
pub struct Event {
    pub entry: Entry,
    pub loc: EntryLocation,
}

#[derive(Debug)]
pub struct EntryLocation {
    remainder: u32, // # of entries in the message after this entry
    partition: i32,
    offset: i64,
}

/// Event reader for Apache Kafka.
pub struct Input {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<EntryLocation>,
    consumer: Consumer,
}

impl Input {
    pub fn new(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<EntryLocation>,
        hosts: Vec<String>,
        group: String,
        client_id: String,
        topic: String,
    ) -> Result<Self, kafka::Error> {
        let consumer = Consumer::from_hosts(hosts)
            .with_group(group)
            .with_fallback_offset(FetchOffset::Earliest)
            .with_fetch_max_bytes_per_partition(1_000_000)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .with_client_id(client_id)
            .with_topic(topic)
            .create()?;
        Ok(Self {
            data_channel: Some(data_channel),
            ack_channel,
            consumer,
        })
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let data_channel = if let Some(channel) = &self.data_channel {
            channel
        } else {
            return Err(Error::ChannelClosed);
        };

        let messagesets = self
            .consumer
            .poll()
            .map_err(|e| Error::CannotFetch(Box::new(e)))?;
        if messagesets.is_empty() {
            return Ok(());
        }

        let mut sel = crossbeam_channel::Select::new();
        let send_data = sel.send(data_channel);
        let recv_ack = sel.recv(&self.ack_channel);
        'messagesets: for msgset in messagesets.iter() {
            let partition = msgset.partition();
            for msg in msgset.messages() {
                let fwd_msg: ForwardMode = rmp_serde::from_slice(msg.value)
                    .map_err(|e| Error::InvalidMessage(Box::new(e)))?;
                if fwd_msg.entries.len() > u32::max_value() as usize {
                    return Err(Error::TooManyEvents(fwd_msg.entries.len()));
                }
                let offset = msg.offset;
                for (remainder, entry) in (0..fwd_msg.entries.len()).rev().zip(fwd_msg.entries) {
                    loop {
                        let oper = sel.select();
                        match oper.index() {
                            i if i == send_data => {
                                let event = Event {
                                    entry,
                                    loc: EntryLocation {
                                        remainder: remainder
                                            .try_into()
                                            .expect("remainder <= u32::max_values()"),
                                        partition,
                                        offset,
                                    },
                                };
                                if oper.send(data_channel, event).is_err() {
                                    // data_channel was disconnected. Exit the
                                    // loop and commit consumed.
                                    break 'messagesets;
                                }
                                break;
                            }
                            i if i == recv_ack => {
                                let ack = if let Ok(ack) = oper.recv(&self.ack_channel) {
                                    ack
                                } else {
                                    // ack_channel was disconnected. Exit the
                                    // loop and commit consumed.
                                    break 'messagesets;
                                };
                                if ack.remainder == 0 {
                                    self.consumer
                                        .consume_message(msgset.topic(), ack.partition, ack.offset)
                                        .map_err(|_| {
                                            Error::Fatal(
                                                "messages from Kafka have different topics".into(),
                                            )
                                        })?;
                                }
                                if self.ack_channel.is_empty() {
                                    self.consumer
                                        .commit_consumed()
                                        .map_err(|e| Error::CannotCommit(Box::new(e)))?;
                                }
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
        }
        self.data_channel = None;
        let topic = self
            .consumer
            .subscriptions()
            .keys()
            .next()
            .expect("subscribes to one topic")
            .clone();
        for ack in &self.ack_channel {
            if ack.remainder == 0 {
                self.consumer
                    .consume_message(&topic, ack.partition, ack.offset)
                    .map_err(|_| {
                        Error::Fatal("messages from Kafka have different topics".into())
                    })?;
            }
        }
        self.consumer
            .commit_consumed()
            .map_err(|e| Error::CannotCommit(Box::new(e)))?;
        Ok(())
    }
}

/// Event writer for Apache Kafka.
pub struct Output<T> {
    data_channel: crossbeam_channel::Receiver<T>,
    producer: Producer,
    topic: String,
}

impl<T> Output<T>
where
    T: std::fmt::Debug + Into<ForwardMode> + Serialize,
{
    pub fn new(
        data_channel: crossbeam_channel::Receiver<T>,
        hosts: Vec<String>,
        topic: String,
    ) -> Result<Self, kafka::Error> {
        let producer = Producer::from_hosts(hosts)
            .with_required_acks(RequiredAcks::One)
            .create()?;
        Ok(Self {
            data_channel,
            producer,
            topic,
        })
    }

    pub fn run(&mut self) -> Result<(), kafka::Error> {
        let mut buf = Vec::new();
        for msg in self.data_channel.iter() {
            msg.serialize(&mut Serializer::new(&mut buf)).map_err(|e| {
                kafka::Error::from_kind(kafka::error::ErrorKind::Msg(format!(
                    "cannot serialize: {}",
                    e
                )))
            })?;
            self.producer
                .send(&Record::from_value(&self.topic, buf.as_slice()))?;
            buf.clear();
        }
        Ok(())
    }
}
