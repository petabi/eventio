use crate::{BareEvent, Error};
use nom::{bytes::complete::tag, IResult};
use std::io::{self, BufRead, BufReader, Read};

pub type Event = BareEvent;

/// Event reader for a text input.
pub struct Input<T: Read> {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<u64>,
    buf: BufReader<T>,
}

impl<T: Read> Input<T> {
    pub fn with_read(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<u64>,
        read: T,
    ) -> Result<Self, Error> {
        let mut buf = BufReader::new(read);
        check_magic_number(&mut buf)?;
        Ok(Self {
            data_channel: Some(data_channel),
            ack_channel,
            buf,
        })
    }
}

fn check_magic_number<T: Read>(reader: &mut BufReader<T>) -> Result<(), Error> {
    let mut buf = vec![];
    reader
        .read_until(b'\n', &mut buf)
        .map_err(|e| Error::CannotFetch(Box::new(e)))?;
    match mbox_magic(&buf) {
        Ok(_) => Ok(()),
        Err(_) => Err(Error::InvalidMessage(Box::new(io::Error::new(
            io::ErrorKind::Other,
            "wrong format",
        )))),
    }
}

fn read_email<T: Read>(reader: &mut BufReader<T>) -> Result<Option<Vec<u8>>, Error> {
    let mut buf = vec![];
    let mut cur = 0;
    loop {
        let bytes = reader
            .read_until(b'\n', &mut buf)
            .map_err(|e| Error::CannotFetch(Box::new(e)))?;
        if bytes == 0 {
            if buf.is_empty() {
                return Ok(None);
            }
            return Ok(Some(buf));
        }
        if mbox_magic(&buf[cur..]).is_ok() {
            buf.resize(cur, 0);
            return Ok(Some(buf));
        } else {
            cur += bytes;
        }
    }
}

impl<T: Read> super::Input for Input<T> {
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
        let mut seq_no = 0;

        'poll: while let Some(email) = read_email(&mut self.buf)? {
            seq_no += 1;
            loop {
                let oper = sel.select();
                match oper.index() {
                    i if i == send_data => {
                        let event = Event { raw: email, seq_no };
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
        self.data_channel = None;
        for _ in &self.ack_channel {}
        Ok(())
    }
}

fn mbox_magic(input: &[u8]) -> IResult<&[u8], &[u8]> {
    tag(b"From ")(input)
}
