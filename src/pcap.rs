//! Reading packets as events from a pcap input.

use crate::{BareEvent, Error};
use std::io::{self, Read};

use pcap_parser::{
    create_reader, data::get_packetdata_ethernet, data::PacketData, traits::PcapReaderIterator,
    Block, PcapBlockOwned, PcapError,
};

/// A packet as a byte sequence;
pub type Event = BareEvent;
const PCAP_BUFFER_SIZE: usize = 65536;

/// Event reader for a pcap input.
pub struct Input<R: Read> {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<u64>,
    iter: Box<dyn PcapReaderIterator<R>>,
}

unsafe impl<R: Read> std::marker::Send for Input<R> {}

impl<R: Read + 'static> Input<R> {
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

impl<R: Read> super::Input for Input<R> {
    type Data = Event;
    type Ack = u64;

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
                        PcapBlockOwned::NG(Block::EnhancedPacket(ref epb)) => {
                            get_packetdata_ethernet(epb.data, epb.caplen as usize)
                        }
                        PcapBlockOwned::NG(Block::SimplePacket(ref spb)) => {
                            get_packetdata_ethernet(spb.data, (spb.block_len1 - 16) as usize)
                        }
                        PcapBlockOwned::NG(_) | PcapBlockOwned::LegacyHeader(_) => None,
                        PcapBlockOwned::Legacy(lpb) => {
                            get_packetdata_ethernet(lpb.data, lpb.caplen as usize)
                        }
                    };

                    if let Some(PacketData::L2(eslice)) = res {
                        id += 1;
                        loop {
                            let oper = sel.select();
                            match oper.index() {
                                i if i == send_data => {
                                    let event = Event {
                                        raw: eslice.to_vec(),
                                        seq_no: id,
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
    use pcap_parser::{LegacyPcapBlock, PcapHeader, ToVec};
    use std::io::Cursor;
    use std::thread;

    fn create_pcap() -> Cursor<Vec<u8>> {
        let fake_content = b"fake packet";
        let pkt = LegacyPcapBlock {
            ts_sec: 0,
            ts_usec: 0,
            caplen: fake_content.len() as u32,
            origlen: fake_content.len() as u32,
            data: fake_content,
        }
        .to_vec_raw()
        .unwrap();
        let mut buf = PcapHeader::new().to_vec_raw().unwrap();
        for _ in (0..10).into_iter() {
            buf.extend(pkt.iter());
        }
        Cursor::new(buf)
    }

    #[test]
    fn pcap_input() {
        let tester = create_pcap();
        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let in_thread = thread::spawn(move || {
            let input = pcap::Input::with_read(data_tx, ack_rx, tester);
            input.run().unwrap()
        });

        let mut events = Vec::new();
        {
            let ack_tx = ack_tx;
            for ev in data_rx {
                events.push(ev.raw);
                ack_tx.send(ev.seq_no).unwrap();
            }
        }
        in_thread.join().unwrap();

        assert_eq!(events.len(), 10);
    }
}
