use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, Serialize, ToSchema)]
pub struct Index {
    pub version: String,
    pub resources: Vec<IndexResource>,
}

#[derive(Deserialize, Serialize, ToSchema)]
pub struct IndexResource {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@type")]
    pub resource_type: String
}

impl Index {
    pub fn new(base_url: &str) -> Index {
        Index {
            version: "3.0.0".to_string(),
            resources: vec![
                Self::create_resource(base_url, "v3/package/", "PackageBaseAddress/3.0.0"),
                Self::create_resource(base_url, "v3/registration/", "RegistrationsBaseUrl"),
                Self::create_resource(base_url, "v3/registration/", "RegistrationsBaseUrl/3.6.0"),
                Self::create_resource(base_url, "v3/search", "SearchQueryService"),
                Self::create_resource(base_url, "v3/search", "SearchQueryService/3.5.0"),
                Self::create_resource(base_url, "v3/search", "SearchQueryService/3.4.0"),
                Self::create_resource(base_url, "v3/search", "SearchQueryService/3.0.0-beta"),
            ]
        }
    }

    fn create_resource(base_url: &str, route: &str, t: &str) -> IndexResource {
        IndexResource {
            id: format!("{}/{}", base_url, route),
            resource_type: t.to_string()
        }
    }
}