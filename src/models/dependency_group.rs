use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Default, Debug, Deserialize, Serialize, ToSchema)]
pub struct DependencyGroup {
    #[serde(rename = "targetFramework", skip_serializing_if = "Option::is_none")]
    pub target_framework: Option<String>,
    pub dependencies: Vec<Dependency>,
}

#[derive(Clone, Default, Debug, Deserialize, Serialize, ToSchema)]
pub struct Dependency {
    pub id: String,
    pub range: String,
}