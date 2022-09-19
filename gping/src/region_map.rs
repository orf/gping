type Host = String;

pub fn try_host_from_cloud_region(query: &str) -> Option<Host> {
    match query.split_once(':') {
        Some(("aws", region)) => Some(format!("ec2.{}.amazonaws.com", region)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_from_aws() {
        assert_eq!(
            try_host_from_cloud_region("aws:eu-west-1"),
            Some("ec2.eu-west-1.amazonaws.com".to_string())
        );
    }

    #[test]
    fn test_host_from_foo() {
        assert_eq!(try_host_from_cloud_region("foo:bar"), None);
    }

    #[test]
    fn test_invalid_input() {
        assert_eq!(try_host_from_cloud_region("foo"), None);
    }
}
