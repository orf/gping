use crate::{run_ping, Parser, PingDetectionError, PingResult, Pinger};
use anyhow::{Context, Result};
use regex::Regex;
use std::{time::Duration, io::{BufRead, BufReader}, thread, sync::mpsc, process::Output};


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
                match run_ping(cmd.as_str(), args) {
                    Ok(output) => {
                        if output.status.success() {
                            if let Some(result) = parser.parse(String::from_utf8(output.stdout).expect("Error decoding stdout")) {
                                if tx.send(result).is_err() {

                                }
                            }
                        } else {
                            let decoded_stderr = String::from_utf8(output.stderr).expect("Error decoding stderr");
                            let _ = tx.send(PingResult::PingExited(output.status, decoded_stderr));
                        }
                    }
                    Err(error) => {
                    }
                };
                thread::sleep(interval);
        }});

        Ok(rx)
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    fn get_interval(&mut self) {
        self.interval.clone();
    }

    fn set_interface(&mut self, interface: Option<String>) {
        self.interface = interface;
    }

    fn ping_args(&self, target: String) -> (String, Vec<String>) {
        // The -O flag ensures we "no answer yet" messages from ping
        // See https://superuser.com/questions/270083/linux-ping-show-time-out
        let mut args = vec![
            "-O".to_string(),
            "-c".to_string(),
            "1".to_string(),
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
        } else if line.starts_with("no answer yet") {
            return Some(PingResult::Timeout(line));
        }
        None
    }
}
