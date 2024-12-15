type Host = String;

pub fn try_host_from_cloud_region(query: &str) -> Option<Host> {
    match query.split_once(':') {
        Some(("aws", region)) => Some(format!("ec2.{region}.amazonaws.com")),
        Some(("gcp", "")) => Some("cloud.google.com".to_string()),
        Some(("gcp", region)) => Some(format!("storage.{region}.rep.googleapis.com")),
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
    fn test_host_from_gcp() {
        assert_eq!(
            try_host_from_cloud_region("gcp:me-central2"),
            Some("storage.me-central2.rep.googleapis.com".to_string())
        );
        assert_eq!(
            try_host_from_cloud_region("gcp:"),
            Some("cloud.google.com".to_string())
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
