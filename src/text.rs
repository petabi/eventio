//! Reading lines as events from a text input.

use crate::{BareEvent, Error};
use std::io::{BufRead, BufReader, Read};

/// A single line as a byte sequence.
pub type Event = BareEvent;

/// Event reader for a text input.
pub struct Input<T: Read> {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<super::SeqNo>,
    buf: BufReader<T>,
}

impl<T: Read> Input<T> {
    pub fn with_read(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<super::SeqNo>,
        read: T,
    ) -> Self {
        Self {
            data_channel: Some(data_channel),
            ack_channel,
            buf: BufReader::new(read),
        }
    }
}

impl<T: Read> super::Input for Input<T> {
    type Data = Event;
    type Ack = super::SeqNo;

    fn run(mut self) -> Result<(), Error> {
        let Some(data_channel) = &self.data_channel else {
            return Err(Error::ChannelClosed);
        };

        let mut sel = crossbeam_channel::Select::new();
        let send_data = sel.send(data_channel);
        let recv_ack = sel.recv(&self.ack_channel);
        let mut line_no = 0;

        'poll: loop {
            let mut line = Vec::new();
            self.buf
                .read_until(b'\n', &mut line)
                .map_err(|e| Error::CannotFetch(Box::new(e)))?;
            if let Some(&b) = line.last() {
                if b == b'\n' {
                    line.pop();
                    if let Some(&b) = line.last() {
                        if b == b'\r' {
                            line.pop();
                        }
                    }
                }
            } else {
                break;
            }
            line_no += 1;
            loop {
                let oper = sel.select();
                match oper.index() {
                    i if i == send_data => {
                        let event = Event {
                            raw: line,
                            seq_no: line_no,
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
        self.data_channel = None;
        for _ in &self.ack_channel {}
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{text, Input};
    use std::thread;

    #[test]
    fn text_input() {
        let text = b"event 1\nevent 2\r\nevent 3";

        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let input = text::Input::with_read(data_tx, ack_rx, text.as_ref());
        let in_thread = thread::spawn(move || input.run().unwrap());

        let mut events = Vec::new();
        {
            let ack_tx = ack_tx;
            for ev in data_rx {
                events.push(ev.raw);
                ack_tx.send(ev.seq_no).unwrap();
            }
        }
        in_thread.join().unwrap();

        assert_eq!(events, [b"event 1", b"event 2", b"event 3"]);
    }
}
