//! A partial implementation of [Fluentd Forward Protocol].
//!
//! [Fluentd Forward Protocol]:
//! https://github.com/fluent/fluentd/wiki/Forward-Protocol-Specification-v1

use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::collections::HashMap;

/// An array representation of pairs of time and record, used in Forward mode.
///
/// See [Entry] in the protocol specification.
///
/// [Entry]:
/// https://github.com/fluent/fluentd/wiki/Forward-Protocol-Specification-v1#entry
#[derive(Debug, Deserialize, Serialize)]
pub struct Entry {
    pub time: u64,
    pub record: HashMap<String, ByteBuf>,
}

/// A series of events packed into a single message.
///
/// See [Forward Mode] in the protocol specification.
///
/// [Forward Mode]:
/// https://github.com/fluent/fluentd/wiki/Forward-Protocol-Specification-v1#forward-mode
#[derive(Debug, Deserialize, Serialize)]
pub struct ForwardMode {
    pub tag: String,
    pub entries: Vec<Entry>,
    pub option: Option<HashMap<String, String>>,
}
