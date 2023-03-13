use crate::{Parser, PingDetectionError, PingResult, PingerTrait, Pinger};
use anyhow::{Context, Result};
use regex::Regex;
use std::{time::Duration, thread, sync::mpsc, process::Output};
use async_process::Command;
use futures::executor;
use tokio::{sync::oneshot, time};

pub async fn run_ping(cmd: &str, args: Vec<String>) -> Result<Output> {
    Command::new(cmd)
        .args(&args)
        // Required to ensure that the output is formatted in the way we expect, not
        // using locale specific delimiters.
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .output().await
        .with_context(|| format!("Failed to run ping with args {:?}", &args))
}

pub fn detect_linux_ping() -> Result<(), PingDetectionError> {
    let output = executor::block_on(run_ping("ping", vec!["-V".to_string()]))?;

    let stdout = String::from_utf8(output.stdout).context("Error decoding ping stdout")?;
    let stderr = String::from_utf8(output.stderr).context("Error decoding ping stderr")?;

    if stdout.contains("iputils") {
        Ok(())
    } else {
        let first_two_lines_stderr: Vec<String> =
            stderr.lines().take(2).map(str::to_string).collect();
        let first_two_lines_stout: Vec<String> =
            stdout.lines().take(2).map(str::to_string).collect();
        Err(PingDetectionError::UnknownPing {
            stdout: first_two_lines_stout,
            stderr: first_two_lines_stderr,
        })
    }
}

#[derive(Default)]
pub struct LinuxPinger {
    interval: Duration,
    interface: Option<String>,
}

impl PingerTrait for LinuxPinger {
    fn start<P>(&self, target: String) -> Result<Pinger>
        where
            P: Parser,
    {
        let args = self.ping_args(target);
        let interval = self.interval;

        let (tx, rx) = mpsc::channel();

        let (notify_exit_sender, exit_receiver) = oneshot::channel();
        let ping_thread = Some((notify_exit_sender,
                                thread::spawn({
                                    let (cmd, args) = args;
                                    move || {
                                        let runtime = tokio::runtime::Builder::new_current_thread()
                                            .enable_all()
                                            .build()
                                            .unwrap();

                                        runtime.spawn(async move {
                                            loop {
                                                let parser = P::default();
                                                match run_ping(cmd.as_str(), args.clone()).await {
                                                    Ok(output) => {
                                                        if output.status.success() {
                                                            if let Some(result) = parser.parse(String::from_utf8(output.stdout.clone()).expect("Error decoding stdout")) {
                                                                if tx.send(result).is_err() {
                                                                    break;
                                                                }
                                                            }
                                                        } else {
                                                            tx.send(PingResult::Failed(output.status.to_string(), "Timeout reached".to_string())).ok();
                                                        }
                                                    }
                                                    Err(e) => {
                                                        panic!("Ping command failed - this should not happen, please verify the integrity of the ping command: {}", e.to_string())
                                                    }
                                                };
                                                time::sleep(interval).await;
                                            }
                                        });

                                        runtime.block_on(async move {
                                            let _ = exit_receiver.await;
                                        });
                                    }
                                })
        ));
        Ok(Pinger {
            channel: rx,
            ping_thread,
        })
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }


    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }

    fn ping_args(&self, target: String) -> (String, Vec<String>) {
        // timeout of 1 second
        let mut args = vec![
            "-c".to_string(),
            "1".to_string(),
            "-W".to_string(),
            "1.0".to_string(),
        ];
        if let Some(interface) = &self.interface {
            args.push("-I".into());
            args.push(interface.clone());
        }
        args.push(target);
        ("ping".to_string(), args)
    }
}

lazy_static! {
    static ref UBUNTU_RE: Regex =
        Regex::new(r"(?i-u)time=(?P<ms>\d+)(?:\.(?P<ns>\d+))? *ms").unwrap();
}

#[derive(Default)]
pub struct LinuxParser {}

impl Parser for LinuxParser {
    fn parse(&self, lines: String) -> Option<PingResult> {
        for line in lines.lines() {
            if line.starts_with("64 bytes from") {
                return self.extract_regex(&UBUNTU_RE, line.to_string());
            }
        }
        return Some(PingResult::Failed("1".to_string(), format!("Failed to parse: {lines}")));
    }
}
