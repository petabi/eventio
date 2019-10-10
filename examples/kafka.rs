use eventio::fluentd::{Entry, ForwardMode};
use eventio::kafka;
use serde_bytes::ByteBuf;
use std::collections::HashMap;
use std::env;
use std::thread;

const TOPIC: &str = "eventio-examples";

fn main() {
    let mut args = env::args();
    args.next().unwrap();
    if let Some(host) = args.next() {
        let hosts = vec![host];
        produce(hosts.clone());
        consume(hosts);
    } else {
        println!("Usage: kafka <kafka_host>");
        std::process::exit(1);
    }
}

fn produce(hosts: Vec<String>) {
    let out_thread = {
        let (tx, rx) = crossbeam_channel::bounded(1);
        let mut output = kafka::Output::new(rx, hosts, TOPIC.into()).unwrap();
        let out_thread = thread::spawn(move || output.run().unwrap());

        let mut record = HashMap::new();
        record.insert("message".into(), ByteBuf::from(b"\x01\x02\x03".to_vec()));
        let entry = Entry {
            time: 123,
            record: record,
        };
        let msg = ForwardMode {
            tag: "tag".into(),
            entries: vec![entry],
            option: None,
        };
        tx.send(msg).unwrap();
        out_thread
    };

    out_thread.join().unwrap();
}

fn consume(hosts: Vec<String>) {
    let (tx, rx) = crossbeam_channel::bounded(1);
    let mut input = kafka::Input::new(
        tx,
        hosts,
        "eventio".into(),
        "eventio-examples".into(),
        TOPIC.into(),
    )
    .unwrap();
    let in_thread = thread::spawn(move || input.run().unwrap());

    let mut entry = Entry {
        time: 0,
        record: HashMap::new(),
    };
    for e in rx {
        entry = e;
    }
    in_thread.join().unwrap();

    assert_eq!(entry.time, 123);
    assert_eq!(entry.record["message"], b"\x01\x02\x03");
}
