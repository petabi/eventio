//! assembling events from matrix

use crate::{BareEvent, Error};
use ndarray::{Array2, Axis};

/// A single line as a byte sequence.
pub type Event = BareEvent;

pub struct Input {
    data_channel: Option<crossbeam_channel::Sender<Event>>,
    ack_channel: crossbeam_channel::Receiver<super::SeqNo>,
    data: Array2<Vec<u8>>,
}

impl Input {
    #[must_use]
    pub fn new(
        data_channel: crossbeam_channel::Sender<Event>,
        ack_channel: crossbeam_channel::Receiver<super::SeqNo>,
        data: Array2<Vec<u8>>,
    ) -> Self {
        Input {
            data_channel: Some(data_channel),
            ack_channel,
            data,
        }
    }
}

impl super::Input for Input {
    type Data = Event;
    type Ack = super::SeqNo;

    fn run(mut self) -> Result<(), Error> {
        let data_channel = if let Some(channel) = &self.data_channel {
            channel
        } else {
            return Err(Error::ChannelClosed);
        };
        let mut sel = crossbeam_channel::Select::new();
        let send_data = sel.send(data_channel);
        let recv_ack = sel.recv(&self.ack_channel);

        'poll: for (idx, row) in self.data.axis_iter(Axis(0)).enumerate() {
            let line = row.fold(Vec::new(), |mut line, col| {
                line.extend_from_slice(col);
                line
            });
            loop {
                let oper = sel.select();
                match oper.index() {
                    i if i == send_data => {
                        let event = Event {
                            raw: line,
                            seq_no: idx,
                        };
                        if oper.send(data_channel, event).is_err() {
                            break 'poll;
                        }
                        break;
                    }
                    i if i == recv_ack => {
                        if oper.recv(&self.ack_channel).is_err() {
                            break 'poll;
                        }
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
    use crate::{ndarray::Input as ndarray_input, Input};
    use ndarray::arr2;
    use std::collections::HashMap;
    use std::thread;

    #[test]
    fn text_input() {
        let data = arr2(&[
            [b"this ".to_vec(), b"is ".to_vec(), b"an ".to_vec()],
            [
                b"event ".to_vec(),
                b"that ".to_vec(),
                b"is splited".to_vec(),
            ],
            [
                b"into multiple ".to_vec(),
                b"weird ".to_vec(),
                b"chunks".to_vec(),
            ],
        ]);

        let ids: HashMap<usize, crate::SeqNo> = [(0, 7), (1, 6), (2, 9)].into_iter().collect();

        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let input = ndarray_input::new(data_tx, ack_rx, data);
        let in_thread = thread::spawn(move || input.run().unwrap());

        let mut events = Vec::new();
        {
            let ack_tx = ack_tx;
            for ev in data_rx {
                let id = ev.seq_no;
                events.push(ev);
                ack_tx.send(id).unwrap();
            }
        }
        in_thread.join().unwrap();

        events.sort_unstable_by_key(|e| ids.get(&e.seq_no).unwrap());
        let raw: Vec<_> = events.into_iter().map(|e| e.raw).collect();

        assert_eq!(
            raw,
            vec![
                b"event that is splited".to_vec(),
                b"this is an ".to_vec(),
                b"into multiple weird chunks".to_vec()
            ]
        );
    }
}
