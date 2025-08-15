#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use crate::bsd::BSDPinger;
    #[cfg(unix)]
    use crate::linux::LinuxPinger;
    #[cfg(unix)]
    use crate::macos::MacOSPinger;
    #[cfg(windows)]
    use crate::windows::WindowsPinger;
    use crate::{PingOptions, PingResult, Pinger};
    use anyhow::bail;
    use ntest::timeout;
    use std::time::Duration;

    const IS_GHA: bool = option_env!("GITHUB_ACTIONS").is_some();

    #[test]
    #[timeout(20_000)]
    fn test_integration_any() {
        run_integration_test(PingOptions::new(
            "tomforb.es",
            Duration::from_millis(500),
            None,
        ))
        .unwrap();
    }
    #[test]
    #[timeout(20_000)]
    fn test_integration_ipv4() {
        run_integration_test(PingOptions::new_ipv4(
            "tomforb.es",
            Duration::from_millis(500),
            None,
        ))
        .unwrap();
    }
    #[test]
    #[timeout(20_000)]
    fn test_integration_ip6() {
        let res = run_integration_test(PingOptions::new_ipv6(
            "tomforb.es",
            Duration::from_millis(500),
            None,
        ));
        // ipv6 tests are allowed to fail on Gitlab CI, as it doesn't support ipv6, apparently.
        if !IS_GHA {
            res.unwrap();
        }
    }

    fn run_integration_test(options: PingOptions) -> anyhow::Result<()> {
        let stream = crate::ping(options.clone())?;

        let mut success = 0;
        let mut errors = 0;

        for message in stream.into_iter().take(3) {
            match message {
                PingResult::Pong(_, m) | PingResult::Timeout(m) => {
                    eprintln!("Message: {}", m);
                    success += 1;
                }
                PingResult::Unknown(line) => {
                    eprintln!("Unknown line: {}", line);
                    errors += 1;
                }
                PingResult::PingExited(code, stderr) => {
                    bail!("Ping exited with code: {}, stderr: {}", code, stderr);
                }
            }
        }
        assert_eq!(success, 3, "Success != 3 with opts {options:?}");
        assert_eq!(errors, 0, "Errors != 0 with opts {options:?}");
        Ok(())
    }

    fn opts() -> PingOptions {
        PingOptions::new("foo".to_string(), Duration::from_secs(1), None)
    }

    fn test_parser<T: Pinger>(contents: &str) {
        let pinger = T::from_options(opts()).unwrap();
        run_parser_test(contents, &pinger);
    }

    fn run_parser_test(contents: &str, pinger: &impl Pinger) {
        let parser = pinger.parse_fn();
        let test_file: Vec<&str> = contents.split("-----").collect();
        let input = test_file[0].trim().split('\n');
        let expected: Vec<&str> = test_file[1].trim().split('\n').collect();
        let parsed: Vec<Option<PingResult>> = input.map(|l| parser(l.to_string())).collect();

        assert_eq!(
            parsed.len(),
            expected.len(),
            "Parsed: {:?}, Expected: {:?}",
            &parsed,
            &expected
        );

        for (idx, (output, expected)) in parsed.into_iter().zip(expected).enumerate() {
            if let Some(value) = output {
                assert_eq!(
                    format!("{value}").trim(),
                    expected.trim(),
                    "Failed at idx {idx}"
                )
            } else {
                assert_eq!("None", expected.trim(), "Failed at idx {idx}")
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn macos() {
        test_parser::<MacOSPinger>(include_str!("tests/macos.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn freebsd() {
        test_parser::<BSDPinger>(include_str!("tests/bsd.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn dragonfly() {
        test_parser::<BSDPinger>(include_str!("tests/bsd.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn openbsd() {
        test_parser::<BSDPinger>(include_str!("tests/bsd.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn netbsd() {
        test_parser::<BSDPinger>(include_str!("tests/bsd.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn ubuntu() {
        run_parser_test(
            include_str!("tests/ubuntu.txt"),
            &LinuxPinger::IPTools(opts()),
        );
    }

    #[cfg(unix)]
    #[test]
    fn debian() {
        run_parser_test(
            include_str!("tests/debian.txt"),
            &LinuxPinger::IPTools(opts()),
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows() {
        test_parser::<WindowsPinger>(include_str!("tests/windows.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn android() {
        run_parser_test(
            include_str!("tests/android.txt"),
            &LinuxPinger::BusyBox(opts()),
        );
    }

    #[cfg(unix)]
    #[test]
    fn alpine() {
        run_parser_test(
            include_str!("tests/alpine.txt"),
            &LinuxPinger::BusyBox(opts()),
        );
    }
}
