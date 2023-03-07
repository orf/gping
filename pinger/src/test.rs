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
        let input = test_file[0].trim();
        let expected: &str = test_file[1].trim();
        let parsed: Option<PingResult> = parser.parse(input.to_string());

        if let Some(value) = parsed {
            assert_eq!(
                format!("{value}").trim(),
                expected.trim(),
                "Failed"
            )
        } else {
            panic!("Could not parse input file")
        }

    }

    #[test]
    fn linux() {
        test_parser::<LinuxParser>(include_str!("tests/linux.txt"));
    }

    #[cfg(windows)]
    #[test]
    fn windows() {
        test_parser::<WindowsParser>(include_str!("tests/windows.txt"));
    }



}
