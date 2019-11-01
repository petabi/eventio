//! Reading lines as events from a text input.

use crate::Error;
use std::io::{BufRead, BufReader, Read};

/// A line in a text input.
#[derive(Debug)]
pub struct Event {
    pub raw: String,
    pub line_no: u64,
}

impl crate::Event for Event {
    type Ack = u64;

    fn raw(&self) -> &[u8] {
        self.raw.as_bytes()
    }

    fn time(&self) -> u64 {
        self.line_no
    }

    fn ack(&self) -> Self::Ack {
        self.line_no
    }
}

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
        let mut line_no = 0;
        'poll: for line in self.buf.lines() {
            let line = line.map_err(|e| Error::CannotFetch(Box::new(e)))?;
            line_no += 1;
            loop {
                let oper = sel.select();
                match oper.index() {
                    i if i == send_data => {
                        let event = Event { raw: line, line_no };
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
        let text = "event 1\nevent 2\nevent 3\n";

        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let input = text::Input::with_read(data_tx, ack_rx, text.as_bytes());
        let in_thread = thread::spawn(move || input.run().unwrap());

        let mut events = Vec::new();
        {
            let ack_tx = ack_tx;
            for ev in data_rx {
                events.push(ev.raw);
                ack_tx.send(ev.line_no).unwrap();
            }
        }
        in_thread.join().unwrap();

        assert_eq!(events, ["event 1", "event 2", "event 3"]);
    }
}
