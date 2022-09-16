use std::collections::HashMap;
use std::fmt;
use std::error::Error;

type Host = String;

#[derive(Debug, Clone, PartialEq)]
pub struct AWSRegionNotFoundError {
	query: String,

}

impl Error for AWSRegionNotFoundError{}


impl fmt::Display for AWSRegionNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid aws region")
    }
}

pub fn try_host_from_aws_region(query: &str) -> Result<Host, AWSRegionNotFoundError> {
    let region_map = HashMap::from([
    	("virginia", "us-east-1"),
    	("ohio", "us-east-2"),
		("california", "us-west-1"),
		("oregon", "us-west-2"),
		("central", "ca-central-1"),
		( "ireland", "eu-west-1"),
		("london", "eu-west-2"),
		("paris", "eu-west-3"),
		("frankfurt", "eu-central-1"),
		("milan", "eu-south-1"),
		("stockholm", "eu-north-1"),
		("bahrain", "me-south-1"),
		("uae", "me-central-1"),
		("cape_town", "af-south-1"),
		("hong_kong", "ap-east-1"),
		("jakarta", "ap-southeast-3"),
		("mumbai", "ap-south-1"),
		("osaka", "ap-northeast-3"),
		("seoul", "ap-northeast-2"),
		("singapore", "ap-southeast-1"),
		("sydney", "ap-southeast-2"),
		("tokyo", "ap-northeast-1"),
		("sao_paulo", "sa-east-1"),
		("beijing", "cn-north-1"),
		("ningxia", "cn-northwest-1"),
		("us-east-1", "us-east-1"),
    	("us-east-2", "us-east-2"),
		("us-west-1", "us-west-1"),
		("us-west-2", "us-west-2"),
		("ca-central-1", "ca-central-1"),
		("eu-west-1", "eu-west-1"),
		("eu-west-2", "eu-west-2"),
		("eu-west-3", "eu-west-3"),
		("eu-central-1", "eu-central-1"),
		("eu-south-1", "eu-south-1"),
		("eu-north-1", "eu-north-1"),
		("me-south-1", "me-south-1"),
		("me-central-1", "me-central-1"),
		("af-south-1", "af-south-1"),
		("ap-east-1", "ap-east-1"),
		("ap-southeast-3", "ap-southeast-3"),
		("ap-south-1", "ap-south-1"),
		("ap-northeast-3", "ap-northeast-3"),
		("p-northeast-2", "ap-northeast-2"),
		("ap-southeast-1", "ap-southeast-1"),
		("ap-southeast-2", "ap-southeast-2"),
		("ap-northeast-1", "ap-northeast-1"),
		("sa-east-1", "sa-east-1"),
		("cn-north-1", "cn-north-1"),
		("cn-northwest-1", "cn-northwest-1"),
    	]);
    let region = region_map.get(query.replace(" ", "_").to_lowercase().as_str());
    match region {
    	None => Err(AWSRegionNotFoundError{query: query.to_string()}),
    	Some(r) => {
    		 		let host: Host = format!("dynamodb.{}.amazonaws.com", r);
    			   Ok(host)
    			}
    }
}

#[cfg(test)]
mod tests {
	use super::*;
    #[test]
    fn test_host_from_city() {
    	assert_eq!(try_host_from_aws_region("Singapore"), Ok("dynamodb.ap-southeast-1.amazonaws.com".to_string()));
    }
    #[test]
    fn test_host_from_imaginary_city() {
    	assert_eq!(try_host_from_aws_region("Atlantis"), Err(AWSRegionNotFoundError{query: "Atlantis".to_string()}));
    }
    #[test]
    fn test_host_from_region_name() {
    	assert_eq!(try_host_from_aws_region("cn-north-1"), Ok("dynamodb.cn-north-1.amazonaws.com".to_string()));
    }
}
