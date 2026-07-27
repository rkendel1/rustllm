use serde::Serialize;

use crate::kernel::manifest::CapabilityManifest;

#[derive(Debug, Clone, Serialize, Default)]
pub struct ExecutionPlan {
    pub capabilities: Vec<CapabilityManifest>,
    pub execution_order: Vec<String>,
    pub missing_dependencies: Vec<String>,
    pub parallel_groups: Vec<Vec<String>>,
}
