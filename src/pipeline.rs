use crate::Event;
use std::thread::{self, JoinHandle};

/// Spawns worker threads to process events in parallel.
pub fn split<D, A, I, O, F, S, R>(
    data_rx: crossbeam_channel::Receiver<D>,
    ack_tx: crossbeam_channel::Sender<A>,
    initialize: I,
    fold: O,
    finalize: F,
) -> Vec<JoinHandle<R>>
where
    D: 'static + Send + Event,
    <D as Event>::Ack: Into<A>,
    A: 'static + Send,
    I: 'static + Fn() -> S + Copy + Send,
    O: 'static + Fn(S, &D) -> S + Copy + Send,
    F: 'static + Fn(S) -> R + Copy + Send,
    R: 'static + Send,
{
    let split_count = num_cpus::get();
    let mut workers = Vec::new();
    let (rx, tx) = (data_rx, ack_tx);
    for _ in 0..split_count {
        let rx = rx.clone();
        let tx = tx.clone();
        workers.push(thread::spawn(move || {
            let mut s = initialize();
            while let Ok(ev) = rx.recv() {
                s = fold(s, &ev);
                if tx.send(ev.ack().into()).is_err() {
                    // The ack channel should not be closed before the data channel.
                    // If that happens, just use the events received so far.
                    break;
                }
            }
            finalize(s)
        }));
    }
    workers
}

#[cfg(test)]
mod tests {
    use crate::{text, Input};
    use std::thread;

    #[test]
    fn split() {
        let text = b"event 1\nevent 2\nevent 3\n";
        let (data_tx, data_rx) = crossbeam_channel::bounded(1);
        let (ack_tx, ack_rx) = crossbeam_channel::bounded(1);
        let input = text::Input::with_read(data_tx, ack_rx, text.as_ref());
        let in_thread = thread::spawn(move || input.run().unwrap());

        let workers = super::split(data_rx, ack_tx, || 0_usize, |sum, _| sum + 1, |x| x);
        in_thread.join().unwrap();
        assert_eq!(
            workers
                .into_iter()
                .map(|w| w.join().unwrap())
                .sum::<usize>(),
            3
        );
    }
}
