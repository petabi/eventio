//! Reading emails as events from an mbox.

use std::io::{self, BufRead, BufReader, Read};

use nom::{bytes::complete::tag, IResult};

use crate::{BareEvent, Error};

/// An email as a byte sequence.
pub type Event = BareEvent;

/// Event reader for a mbox input.
pub struct Input<T: Read> {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<super::SeqNo>,
    buf: BufReader<T>,
}

impl<T: Read> Input<T> {
    /// Creates `Input` that reads emails from mbox.
    ///
    /// # Errors
    ///
    /// Returns an error if `read` is not a valid mbox.
    pub fn with_read(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<super::SeqNo>,
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
        }
        cur += bytes;
    }
}

impl<T: Read> super::Input for Input<T> {
    type Data = Event;
    type Ack = super::SeqNo;

    /// Reads emails from mbox and forwards them through `data_channel`.
    ///
    /// # Errors
    ///
    /// Returns an error if reading an email from mbox fails.
    fn run(mut self) -> Result<(), Error> {
        let Some(data_channel) = &self.data_channel else {
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
    tag(&b"From "[..])(input)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::thread;

    use crate::Input;

    fn read_emails(text: &'static [u8]) -> Result<Vec<super::Event>, super::Error> {
        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let cursor: Cursor<&[u8]> = Cursor::new(text);

        let input = super::Input::with_read(data_tx, ack_rx, cursor)?;
        let in_thread = thread::spawn(move || input.run().unwrap());

        let mut events = Vec::new();
        {
            let ack_tx = ack_tx;
            for ev in data_rx {
                ack_tx.send(ev.seq_no).unwrap();
                events.push(ev);
            }
        }
        in_thread.join().unwrap();

        Ok(events)
    }

    #[test]
    fn empty() {
        let text = b"";
        assert!(read_emails(text).is_err());
    }

    #[test]
    fn end_of_email() {
        let text = b"From \r\n\r\n";
        let events = read_emails(text).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn not_corrupted() {
        let text = b"From valid \n\nFor...\n";

        let events = read_emails(text).unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn corrupted() {
        let text = b"Fr something else\r\nFrom \r\n\r\n";
        assert!(read_emails(text).is_err());
    }

    #[test]
    fn two_emails() {
        let text = b"From \r\n\r\nFrom \r\n\r\n";
        let res = read_emails(text).unwrap();
        assert_eq!(res.len(), 2);
    }
}
