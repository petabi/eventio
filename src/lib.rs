//! Channel-based event I/O processors.
//!
//! This crate allows to collect events from Apache Kafka servers, distribute
//! them to multiple threads, and optionally collect them to send to Kafka.

pub mod fluentd;
pub mod kafka;

use std::error;
use std::fmt;

/// A trait for a data source that produces messages of type `S`.
pub trait Input {
    fn run(&mut self) -> Result<(), Error>;
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
