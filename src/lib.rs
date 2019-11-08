//! Channel-based event I/O processors.
//!
//! This crate allows to collect events from text files, pcap files, and Apache
//! Kafka servers, distribute them to multiple threads, and optionally collect
//! them to send to Kafka.

pub mod fluentd;
pub mod kafka;
pub mod pcap;
mod pipeline;
pub mod text;

pub use pipeline::split;
use std::error;
use std::fmt;

/// A trait for a data source that produces messages of type `Data`.
pub trait Input {
    type Data;
    type Ack;

    /// Fetches events and send them as `Data`. It also receives and processes
    /// `Ack`, which acknowledges the receipt of a certain `Data`.
    fn run(self) -> Result<(), Error>;
}

/// A trait for a single event from any type of data source.
pub trait Event {
    type Ack;

    fn raw(&self) -> &[u8];
    fn time(&self) -> u64;
    fn ack(&self) -> Self::Ack;
}

#[derive(Debug)]
pub struct BareEvent {
    pub raw: Vec<u8>,
    pub seq_no: u64,
}

impl Event for BareEvent {
    type Ack = u64;

    fn raw(&self) -> &[u8] {
        self.raw.as_slice()
    }

    fn time(&self) -> u64 {
        self.seq_no
    }

    fn ack(&self) -> Self::Ack {
        self.seq_no
    }
}

/// The error type for event I/O operations.
#[derive(Debug)]
pub enum Error {
    /// The data channel was closed.
    ChannelClosed,
    /// Cannot commit consumed events to the source.
    CannotCommit(Box<dyn error::Error>),
    /// Cannot fetch events from the source.
    CannotFetch(Box<dyn error::Error>),
    /// Cannot parse a message.
    InvalidMessage(Box<dyn error::Error>),
    /// An internal error that should not occur.
    Fatal(String),
    /// Too many events to handle.
    TooManyEvents(usize),
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::CannotCommit(e) | Self::CannotFetch(e) | Self::InvalidMessage(e) => {
                Some(e.as_ref())
            }
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ChannelClosed => write!(f, "data channel closed"),
            Self::CannotCommit(e) => write!(f, "cannot commit to Kafka: {}", e),
            Self::CannotFetch(e) => write!(f, "cannot fetch message from Kafka: {}", e),
            Self::InvalidMessage(e) => write!(f, "invalid MessagePack format: {}", e),
            Self::Fatal(s) => write!(f, "fatal error: {}", s),
            Self::TooManyEvents(n) => write!(
                f,
                "cannot handle {} events (expected < {})",
                n,
                u32::max_value()
            ),
        }
    }
}
