use crate::{PingCreationError, PingOptions, PingResult, Pinger};
use rand::prelude::*;
use rand::rng;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;

pub struct FakePinger {
    options: PingOptions,
}

impl Pinger for FakePinger {
    fn from_options(options: PingOptions) -> Result<Self, PingCreationError>
    where
        Self: Sized,
    {
        Ok(Self { options })
    }

    fn parse_fn(&self) -> fn(String) -> Option<PingResult> {
        unimplemented!("parse for FakeParser not implemented")
    }

    fn ping_args(&self) -> (&str, Vec<String>) {
        unimplemented!("ping_args not implemented for FakePinger")
    }

    fn start(&self) -> Result<Receiver<PingResult>, PingCreationError> {
        let (tx, rx) = mpsc::channel();
        let sleep_time = self.options.interval;

        thread::spawn(move || {
            let mut random = rng();
            loop {
                let fake_seconds = random.random_range(50..150);
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
}
