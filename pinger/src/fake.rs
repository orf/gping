use crate::{Parser, PingResult, Pinger};
use rand::prelude::*;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

pub struct FakePinger {
    interval: Duration,
}

impl Pinger for FakePinger {
    type Parser = FakeParser;

    fn new(interval: Duration, _interface: Option<String>) -> Self {
        Self { interval }
    }

    fn start(&self, _target: String) -> anyhow::Result<Receiver<PingResult>> {
        let (tx, rx) = mpsc::channel();
        let sleep_time = self.interval;

        thread::spawn(move || {
            let mut random = rand::thread_rng();
            loop {
                let fake_seconds = random.gen_range(50..150);
                let ping_result = PingResult::Pong(
                    Duration::from_millis(fake_seconds),
                    format!("Fake ping line: {fake_seconds} ms"),
                );
                if tx.send(ping_result).is_err() {
                    break;
                }

                std::thread::sleep(sleep_time);
            }
        });

        Ok(rx)
    }

    fn ping_args(&self, _target: String) -> (&str, Vec<String>) {
        unimplemented!("ping_args not implemented for FakePinger")
    }
}

#[derive(Default)]
pub struct FakeParser {}

impl Parser for FakeParser {
    fn parse(&self, _line: String) -> Option<PingResult> {
        unimplemented!("parse for FakeParser not implemented")
    }
}
