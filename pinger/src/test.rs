#[cfg(test)]
mod tests {
    use crate::linux::LinuxParser;
    use crate::{Parser, PingResult};

    #[cfg(windows)]
    use crate::windows::WindowsParser;

    fn test_parser<T>(contents: &str)
    where
        T: Parser,
    {
        let parser = T::default();
        let test_file: Vec<&str> = contents.split("-----").collect();
        let input = test_file[0].trim().split('\n');
        let expected: Vec<&str> = test_file[1].trim().split('\n').collect();
        let parsed: Vec<Option<PingResult>> = input.map(|l| parser.parse(l.to_string())).collect();

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

    #[test]
    fn ubuntu() {
        test_parser::<LinuxParser>(include_str!("tests/ubuntu.txt"));
    }

    #[test]
    fn debian() {
        test_parser::<LinuxParser>(include_str!("tests/debian.txt"));
    }

    #[cfg(windows)]
    #[test]
    fn windows() {
        test_parser::<WindowsParser>(include_str!("tests/windows.txt"));
    }

    #[test]
    fn android() {
        test_parser::<LinuxParser>(include_str!("tests/android.txt"));
    }

    #[test]
    fn alpine() {
        test_parser::<LinuxParser>(include_str!("tests/alpine.txt"));
    }
}
