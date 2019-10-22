//! Reading packets as events from a pcap file.

use crate::Error;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use pcap_parser::{
    create_reader, data::get_packetdata_ethernet, data::PacketData, traits::PcapReaderIterator,
    Block, PcapBlockOwned, PcapError,
};

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

        'poll: loop {
            match self.iter.next() {
                Ok((offset, block)) => {
                    let res = match block {
                        PcapBlockOwned::NG(Block::SectionHeader(ref _shb)) => None,
                        PcapBlockOwned::NG(Block::InterfaceDescription(ref _idb)) => None,
                        PcapBlockOwned::NG(Block::EnhancedPacket(ref epb)) => {
                            get_packetdata_ethernet(epb.data, epb.caplen as usize)
                        }
                        PcapBlockOwned::NG(Block::SimplePacket(ref spb)) => {
                            get_packetdata_ethernet(spb.data, (spb.block_len1 - 16) as usize)
                        }
                        PcapBlockOwned::NG(_) => None,
                        PcapBlockOwned::Legacy(lpb) => {
                            get_packetdata_ethernet(lpb.data, lpb.caplen as usize)
                        }
                        PcapBlockOwned::LegacyHeader(_) => None,
                    };

                    if let Some(PacketData::L2(eslice)) = res {
                        id += 1;
                        loop {
                            let oper = sel.select();
                            match oper.index() {
                                i if i == send_data => {
                                    let event = Event {
                                        raw: eslice.to_vec(),
                                        id,
                                    };
                                    if oper.send(data_channel, event).is_err() {
                                        // data_channel was disconnected. Exit the
                                        // loop and commit consumed.
                                        break 'poll;
                                    }
                                    break;
                                }
                                i if i == recv_ack => {
                                    if oper.recv(&self.ack_channel).is_err() {
                                        // ack_channel was disconnected. Exit the
                                        // loop and commit consumed.
                                        break 'poll;
                                    };
                                }
                                _ => unreachable!(),
                            }
                        }
                    }
                    self.iter.consume_noshift(offset);
                }
                Err(PcapError::Eof) => break 'poll,
                Err(PcapError::Incomplete) => {
                    self.iter.refill().map_err(|e| {
                        Error::CannotFetch(Box::new(io::Error::new(
                            io::ErrorKind::Other,
                            format!("cannot read packet from pcap: {:?}", e),
                        )))
                    })?;
                }
                Err(e) => {
                    return Err(Error::CannotFetch(Box::new(io::Error::new(
                        io::ErrorKind::Other,
                        format!("cannot read packet from pcap: {:?}", e),
                    ))));
                }
            }
        }
        self.data_channel = None;
        for _ in &self.ack_channel {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{pcap, Input};
    use mktemp::Temp;
    use pcap_file::{Packet, PcapWriter};
    use std::fs::File;
    use std::thread;

    fn test_pcap() -> Temp {
        let tester = Temp::new_path();
        let file = File::create(tester.as_path()).expect("Error creating file");
        let mut pcap_writer = PcapWriter::new(file).unwrap();
        let fake_content = b"fake packet";
        let pkt = Packet::new(0, 0, fake_content.len() as u32, fake_content);
        for _ in (0..10).into_iter() {
            pcap_writer.write_packet(&pkt).unwrap();
        }
        tester
    }

    #[test]
    fn text_input() {
        let tester = test_pcap();
        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let in_thread = thread::spawn(move || {
            let input = pcap::Input::<File>::with_path(data_tx, ack_rx, tester.as_path()).unwrap();
            input.run().unwrap()
        });

        let mut events = Vec::new();
        {
            let ack_tx = ack_tx;
            for ev in data_rx {
                events.push(ev.raw);
                ack_tx.send(ev.id).unwrap();
            }
        }
        in_thread.join().unwrap();

        assert_eq!(events.len(), 10);
    }
}
