//! Reading/writing events from/to Apache Kafka servers.

use crate::fluentd::ForwardMode;
use crate::Event;
use kafka::consumer::{Consumer, FetchOffset, GroupOffsetStorage};
use kafka::error::{Error, ErrorKind};
use kafka::producer::{Producer, Record, RequiredAcks};
use rmp_serde::Serializer;
use serde::Serialize;

/// Event reader for Apache Kafka.
pub struct Input {
    channel: crossbeam_channel::Sender<Event<u8>>,
    consumer: Consumer,
}

impl Input {
    pub fn new(
        channel: crossbeam_channel::Sender<Event<u8>>,
        hosts: Vec<String>,
        group: String,
        client_id: String,
        topic: String,
    ) -> Result<Self, Error> {
        let consumer = Consumer::from_hosts(hosts)
            .with_group(group)
            .with_fallback_offset(FetchOffset::Earliest)
            .with_fetch_max_bytes_per_partition(1_000_000)
            .with_offset_storage(GroupOffsetStorage::Kafka)
            .with_client_id(client_id)
            .with_topic(topic)
            .create()?;
        Ok(Self { channel, consumer })
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let messagesets = self.consumer.poll()?;
        if messagesets.is_empty() {
            return Ok(());
        }
        for msgset in messagesets.iter() {
            for msg in msgset.messages() {
                let fwd_msg = (rmp_serde::from_slice(msg.value)
                    as Result<ForwardMode, rmp_serde::decode::Error>)
                    .map_err(|e| {
                        Error::from_kind(ErrorKind::Msg(format!(
                            "invalid MessagePack message: {}",
                            e
                        )))
                    })?;
                for entry in fwd_msg.entries {
                    let event = entry
                        .record
                        .get("message")
                        .map(|v| Event::from_slice(entry.time, v.as_slice()))
                        .ok_or_else(|| {
                            Error::from_kind(ErrorKind::Msg("no message field in record".into()))
                        })?;
                    self.channel.send(event).map_err(|e| {
                        Error::from_kind(ErrorKind::Msg(format!(
                            "cannot send message through channel: {}",
                            e
                        )))
                    })?;
                }
            }
            self.consumer.consume_messageset(msgset)?;
        }
        self.consumer.commit_consumed()?;
        Ok(())
    }
}

/// Event writer for Apache Kafka.
pub struct Output<T> {
    channel: crossbeam_channel::Receiver<T>,
    producer: Producer,
    topic: String,
}

impl<T> Output<T>
where
    T: std::fmt::Debug + Into<ForwardMode> + Serialize,
{
    pub fn new(
        channel: crossbeam_channel::Receiver<T>,
        hosts: Vec<String>,
        topic: String,
    ) -> Result<Self, Error> {
        let producer = Producer::from_hosts(hosts)
            .with_required_acks(RequiredAcks::One)
            .create()?;
        Ok(Self {
            channel,
            producer,
            topic,
        })
    }

    pub fn run(&mut self) -> Result<(), Error> {
        let mut buf = Vec::new();
        for msg in self.channel.iter() {
            msg.serialize(&mut Serializer::new(&mut buf)).map_err(|e| {
                Error::from_kind(ErrorKind::Msg(format!("cannot serialize: {}", e)))
            })?;
            self.producer
                .send(&Record::from_value(&self.topic, buf.as_slice()))?;
            buf.clear();
        }
        Ok(())
    }
}
