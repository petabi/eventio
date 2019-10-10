//! Channel-based event I/O processors.
//!
//! This crate allows to collect events from Apache Kafka servers, distribute
//! them to multiple threads, and optionally collect them to send to Kafka.

pub mod fluentd;
pub mod kafka;
