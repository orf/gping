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
    	("aws:virginia", "us-east-1"),
    	("aws:ohio", "us-east-2"),
		("aws:california", "us-west-1"),
		("aws:oregon", "us-west-2"),
		("aws:central", "ca-central-1"),
		("aws:ireland", "eu-west-1"),
		("aws:london", "eu-west-2"),
		("aws:paris", "eu-west-3"),
		("aws:frankfurt", "eu-central-1"),
		("aws:milan", "eu-south-1"),
		("aws:stockholm", "eu-north-1"),
		("aws:bahrain", "me-south-1"),
		("aws:uae", "me-central-1"),
		("aws:cape_town", "af-south-1"),
		("aws:hong_kong", "ap-east-1"),
		("aws:jakarta", "ap-southeast-3"),
		("aws:mumbai", "ap-south-1"),
		("aws:osaka", "ap-northeast-3"),
		("aws:seoul", "ap-northeast-2"),
		("aws:singapore", "ap-southeast-1"),
		("aws:sydney", "ap-southeast-2"),
		("aws:tokyo", "ap-northeast-1"),
		("aws:sao_paulo", "sa-east-1"),
		("aws:beijing", "cn-north-1"),
		("aws:ningxia", "cn-northwest-1"),
		("aws:us-east-1", "us-east-1"),
		("aws:us-east-2", "us-east-2"),
		("aws:us-west-1", "us-west-1"),
		("aws:us-west-2", "us-west-2"),
		("aws:ca-central-1", "ca-central-1"),
		("aws:eu-west-1", "eu-west-1"),
		("aws:eu-west-2", "eu-west-2"),
		("aws:eu-west-3", "eu-west-3"),
		("aws:eu-central-1", "eu-central-1"),
		("aws:eu-south-1", "eu-south-1"),
		("aws:eu-north-1", "eu-north-1"),
		("aws:me-south-1", "me-south-1"),
		("aws:me-central-1", "me-central-1"),
		("aws:af-south-1", "af-south-1"),
		("aws:ap-east-1", "ap-east-1"),
		("aws:ap-southeast-3", "ap-southeast-3"),
		("aws:ap-south-1", "ap-south-1"),
		("aws:ap-northeast-3", "ap-northeast-3"),
		("aws:p-northeast-2", "ap-northeast-2"),
		("aws:ap-southeast-1", "ap-southeast-1"),
		("aws:ap-southeast-2", "ap-southeast-2"),
		("aws:ap-northeast-1", "ap-northeast-1"),
		("aws:sa-east-1", "sa-east-1"),
		("aws:cn-north-1", "cn-north-1"),
		("aws:cn-northwest-1", "cn-northwest-1"),
    	]);
    let region = region_map.get(query.replace(" ", "_").to_lowercase().as_str());
    match region {
    	None => Err(AWSRegionNotFoundError{query: query.to_string()}),
    	Some(r) => {
    		 		let host: Host = format!("ec2.{}.amazonaws.com", r);
    			   Ok(host)
    			}
    }
}

#[cfg(test)]
mod tests {
	use super::*;
    #[test]
    fn test_host_from_city() {
    	assert_eq!(try_host_from_aws_region("aws:Singapore"), Ok("ec2.ap-southeast-1.amazonaws.com".to_string()));
    }
    #[test]
    fn test_host_from_imaginary_city() {
    	assert_eq!(try_host_from_aws_region("Atlantis"), Err(AWSRegionNotFoundError{query: "Atlantis".to_string()}));
    }
    #[test]
    fn test_host_from_region_name() {
    	assert_eq!(try_host_from_aws_region("aws:cn-north-1"), Ok("ec2.cn-north-1.amazonaws.com".to_string()));
    }
}
