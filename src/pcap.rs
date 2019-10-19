//! Reading packets as events from a pcap file.

use crate::Error;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use pcap_parser::{create_reader, traits::PcapReaderIterator};

#[derive(Debug)]
pub struct Event {
    pub raw: Vec<u8>,
    pub id: u64,
}

const PCAP_BUFFER_SIZE: usize = 65536;

pub struct Input<'a, R: Read + 'a> {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<u64>,
    iter: Box<dyn PcapReaderIterator<R> + 'a>,
}

impl<'a, R: Read + 'a> Input<'a, R> {
    pub fn with_path<P: AsRef<Path>>(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<u64>,
        path: P,
    ) -> io::Result<Input<'a, File>> {
        let file = File::open(path.as_ref())?;
        let iter = create_reader(PCAP_BUFFER_SIZE, file)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "pcap error"))?;

        Ok(Input {
            data_channel: Some(data_channel),
            ack_channel,
            iter,
        })
    }

    pub fn with_read(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<u64>,
        read: R,
    ) -> Self {
        Self {
            data_channel: Some(data_channel),
            ack_channel,
            iter: create_reader(PCAP_BUFFER_SIZE, read).expect("pcap error"),
        }
    }
}

impl<'a, R: Read + 'a> super::Input for Input<'a, R> {
    fn run(mut self) -> Result<(), Error> {
        let data_channel = if let Some(channel) = &self.data_channel {
            channel
        } else {
            return Err(Error::ChannelClosed);
        };
        let mut sel = crossbeam_channel::Select::new();
        let send_data = sel.send(data_channel);
        let recv_ack = sel.recv(&self.ack_channel);
        let mut id = 0;

        Ok(())
    }
}
