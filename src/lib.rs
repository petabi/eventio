//! Channel-based event I/O processors.
//!
//! This crate allows to collect events from Apache Kafka servers, distribute
//! them to multiple threads, and optionally collect them to send to Kafka.

pub mod fluentd;
pub mod kafka;

use serde::{Deserialize, Serialize};

/// A raw event with an identifier.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Event<T> {
    /// Identifier.
    ///
    /// Allows any 64-bit integer. If the source is [`fluentd::ForwardMode`],
    /// this is a copy of the [`fluentd::Entry::time`] of the corresponding
    /// elements in [`fluentd::ForwardMode::entries`].
    pub id: u64,

    /// Raw data.
    ///
    /// The most common type for this field is `Vec<u8>` that represents an
    /// event as a sequence of bytes.
    pub data: Vec<T>,
}

impl<T> Event<T>
where
    T: Copy,
{
    /// Creates an event from an identifier and raw data.
    pub fn from_slice(id: u64, slice: &[T]) -> Self {
        Self {
            id,
            data: slice.to_vec(),
        }
    }
}
