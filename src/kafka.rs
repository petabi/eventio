//! Reading/writing events from/to Apache Kafka servers.

use crate::fluentd::{Entry, ForwardMode};
use crate::Error;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::producer::{Producer, Record, RequiredAcks};
use rmp_serde::Serializer;
use serde::Serialize;
use std::convert::TryInto;

/// An event included in a Kafka message at `loc`.
#[derive(Debug)]
pub struct Event {
    pub entry: Entry,
    pub loc: EntryLocation,
}

impl crate::Event for Event {
    type Ack = EntryLocation;

    #[must_use]
    fn raw(&self) -> &[u8] {
        self.entry
            .record
            .get("message")
            .map_or(b"", |v| v.as_slice())
    }

    #[must_use]
    fn time(&self) -> u64 {
        self.entry.time
    }

    #[must_use]
    fn ack(&self) -> Self::Ack {
        self.loc
    }
}

/// The location of an event on a Kafka topic.
#[derive(Copy, Clone, Debug)]
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
    fetch_limit: usize,
}

impl Input {
    /// Creates `Input` that fetches at most `fetch_limit` entries from the
    /// given Kafka topic.
    ///
    /// # Errors
    ///
    /// Returns an error if it fails to connect to Kafka as a consumer.
    pub fn new(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<EntryLocation>,
        hosts: Vec<String>,
        group: String,
        client_id: String,
        topic: String,
        fetch_limit: usize,
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
            fetch_limit,
        })
    }
}

impl super::Input for Input {
    type Data = Event;
    type Ack = EntryLocation;

    /// Reads events from Kafak and forwards them through `data_channel`.
    ///
    /// # Errors
    ///
    /// Returns an error if it cannot fetch messages from Kafka, receives an
    /// invalid message, or receives an invalid ACK from `ack_channel`.
    fn run(mut self) -> Result<(), Error> {
        let data_channel = if let Some(channel) = &self.data_channel {
            channel
        } else {
            return Err(Error::ChannelClosed);
        };

        let mut sel = crossbeam_channel::Select::new();
        let send_data = sel.send(data_channel);
        let recv_ack = sel.recv(&self.ack_channel);

        'poll: loop {
            let messagesets = self
                .consumer
                .poll()
                .map_err(|e| Error::CannotFetch(Box::new(e)))?;
            if messagesets.is_empty() {
                break 'poll;
            }
            for msgset in messagesets.iter() {
                let partition = msgset.partition();
                for msg in msgset.messages() {
                    let fwd_msg: ForwardMode = rmp_serde::from_slice(msg.value)
                        .map_err(|e| Error::InvalidMessage(Box::new(e)))?;
                    if fwd_msg.entries.len() > u32::max_value() as usize {
                        return Err(Error::TooManyEvents(fwd_msg.entries.len()));
                    }
                    let (remaining, overflow) =
                        self.fetch_limit.overflowing_sub(fwd_msg.entries.len());
                    if overflow {
                        break 'poll;
                    } else {
                        self.fetch_limit = remaining;
                    }
                    let offset = msg.offset;
                    for (remainder, entry) in (0..fwd_msg.entries.len()).rev().zip(fwd_msg.entries)
                    {
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
                                        break 'poll;
                                    }
                                    break;
                                }
                                i if i == recv_ack => {
                                    let ack = if let Ok(ack) = oper.recv(&self.ack_channel) {
                                        ack
                                    } else {
                                        // ack_channel was disconnected. Exit the
                                        // loop and commit consumed.
                                        break 'poll;
                                    };
                                    handle_ack(
                                        &self.ack_channel,
                                        &mut self.consumer,
                                        msgset.topic(),
                                        &ack,
                                    )?;
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                }
            }
        }
        self.data_channel = None;
        let subs = self.consumer.subscriptions();
        let topic = subs.keys().next().expect("subscribes to one topic");
        for ack in &self.ack_channel {
            handle_ack(&self.ack_channel, &mut self.consumer, topic, &ack)?;
        }
        Ok(())
    }
}

fn handle_ack(
    ack_channel: &crossbeam_channel::Receiver<EntryLocation>,
    consumer: &mut Consumer,
    topic: &str,
    ack: &EntryLocation,
) -> Result<(), Error> {
    if ack.remainder == 0 {
        consumer
            .consume_message(topic, ack.partition, ack.offset)
            .map_err(|e| {
                Error::Fatal(format!("messages from Kafka have different topics: {}", e))
            })?;
    }
    if ack_channel.is_empty() {
        consumer
            .commit_consumed()
            .map_err(|e| Error::CannotCommit(Box::new(e)))?;
    }
    Ok(())
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
    /// Creates an event writer for Apache Kafka.
    ///
    /// # Errors
    ///
    /// Returns an error if it fails to connect to Kafka as a consumer.
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

    /// Sends messages received through `data_channel` to Kafka.
    ///
    /// # Errors
    ///
    /// Returns an error if message serialization or transmission fails.
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
