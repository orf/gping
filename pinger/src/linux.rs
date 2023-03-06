use crate::{run_ping, Parser, PingDetectionError, PingResult, Pinger};
use anyhow::{Context, Result};
use regex::Regex;
use std::{time::Duration, thread, sync::mpsc};
use std::process::ExitStatus;


pub fn detect_linux_ping() -> Result<(), PingDetectionError> {
    let output = run_ping("ping", vec!["-V".to_string()])?;
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

impl Pinger for LinuxPinger {

    fn start<P>(&self, target: String) -> Result<mpsc::Receiver<PingResult>>
        where
            P: Parser,
    {

        let args = self.ping_args(target);
        let interval = self.interval.clone();

        let (tx, rx) = mpsc::channel();
        thread::spawn({
            let (cmd, args) = args.clone();
            move || {
                let parser = P::default();
                loop {
                    match run_ping(cmd.as_str(), args.clone()) {
                        Ok(output) => {
                            let outy = String::from_utf8(output.stdout.clone());
                            if output.status.success() {
                                if let Some(result) = parser.parse(String::from_utf8(output.stdout.clone()).expect("Error decoding stdout")) {
                                    println!("{:?}", output.stdout.clone());
                                    if tx.send(result).is_err() {
                                        break;
                                    }
                                }
                            } else {
                                let decoded_stderr = String::from_utf8(output.stderr).expect("Error decoding stderr");
                                let _ = tx.send(PingResult::Failed(output.status.to_string(), decoded_stderr));
                            }
                        }
                        Err(_) => {
                            panic!("Ping command failed - this should not happen")
                        }
                    };
                    thread::sleep(interval);
                }
        }});

        Ok(rx)
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }


    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }

    fn ping_args(&self, target: String) -> (String, Vec<String>) {
        // The -O flag ensures we "no answer yet" messages from ping
        // See https://superuser.com/questions/270083/linux-ping-show-time-out
        let mut args = vec![
            "-c".to_string(),
            "1".to_string(),
            "-W".to_string(),
            format!("{}", self.interval.as_millis() as f32 / 1_000_f32),
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
    fn parse(&self, line: String) -> Option<PingResult> {

        if line.starts_with("64 bytes from") {
            return self.extract_regex(&UBUNTU_RE, line);
        } else {
            return Some(PingResult::Failed("1".to_string(), format!("Failed to parse: {}", line)));
        }

    }
}
