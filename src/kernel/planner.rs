use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Result, anyhow};

use crate::runtime::execution_plan::ExecutionPlan;

use super::manifest::CapabilityManifest;

const ALLOWED_PERMISSIONS: &[&str] = &[
    "identity.read",
    "policy.enforce",
    "budget.manage",
    "provider.route",
    "metadata.read",
    "metadata.write",
    "tools.use",
    "plugins.execute",
    "guardrails.scan",
];

pub struct ExecutionPlanner;

impl ExecutionPlanner {
    pub fn build_plan(manifests: Vec<CapabilityManifest>) -> Result<ExecutionPlan> {
        let mut by_id = HashMap::new();
        let mut provider_map: HashMap<String, Vec<String>> = HashMap::new();

        for manifest in manifests {
            manifest.validate()?;
            if by_id
                .insert(manifest.id.clone(), manifest.clone())
                .is_some()
            {
                return Err(anyhow!("duplicate capability id '{}'", manifest.id));
            }
            for provided in &manifest.provides {
                provider_map
                    .entry(provided.clone())
                    .or_default()
                    .push(manifest.id.clone());
            }
        }

        for manifest in by_id.values() {
            for permission in &manifest.permissions {
                if !ALLOWED_PERMISSIONS.contains(&permission.as_str()) {
                    return Err(anyhow!(
                        "capability '{}' declares invalid permission '{}'",
                        manifest.id,
                        permission
                    ));
                }
            }
        }

        let mut adjacency: HashMap<String, BTreeSet<String>> = HashMap::new();
        let mut indegree: HashMap<String, usize> = HashMap::new();
        let mut missing_dependencies = Vec::new();

        for id in by_id.keys() {
            adjacency.entry(id.clone()).or_default();
            indegree.entry(id.clone()).or_insert(0);
        }

        for manifest in by_id.values() {
            for requirement in &manifest.requires {
                let providers = provider_map.get(requirement);
                let Some(providers) = providers else {
                    missing_dependencies.push(format!(
                        "capability '{}' requires '{}' but no provider is registered",
                        manifest.id, requirement
                    ));
                    continue;
                };
                for provider in providers {
                    Self::add_edge(provider, &manifest.id, &mut adjacency, &mut indegree);
                }
            }

            for target in &manifest.before {
                if !by_id.contains_key(target) {
                    return Err(anyhow!(
                        "capability '{}' declares before '{}' which is unknown",
                        manifest.id,
                        target
                    ));
                }
                Self::add_edge(&manifest.id, target, &mut adjacency, &mut indegree);
            }

            for target in &manifest.after {
                if !by_id.contains_key(target) {
                    return Err(anyhow!(
                        "capability '{}' declares after '{}' which is unknown",
                        manifest.id,
                        target
                    ));
                }
                Self::add_edge(target, &manifest.id, &mut adjacency, &mut indegree);
            }
        }

        if !missing_dependencies.is_empty() {
            return Ok(ExecutionPlan {
                capabilities: by_id.values().cloned().collect(),
                execution_order: Vec::new(),
                missing_dependencies,
                parallel_groups: Vec::new(),
            });
        }

        let mut remaining = indegree;
        let mut scheduled = HashSet::new();
        let mut execution_order = Vec::new();
        let mut parallel_groups = Vec::new();

        loop {
            let mut ready = remaining
                .iter()
                .filter_map(|(id, degree)| {
                    if *degree == 0 && !scheduled.contains(id) {
                        Some(id.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<String>>();

            if ready.is_empty() {
                break;
            }

            ready.sort();
            for id in &ready {
                scheduled.insert(id.clone());
                execution_order.push(id.clone());
            }

            for id in &ready {
                if let Some(neighbors) = adjacency.get(id) {
                    for neighbor in neighbors {
                        if let Some(entry) = remaining.get_mut(neighbor)
                            && *entry > 0
                        {
                            *entry -= 1;
                        }
                    }
                }
            }

            parallel_groups.push(ready);
        }

        if execution_order.len() != by_id.len() {
            return Err(anyhow!("cyclic capability graph detected"));
        }

        Ok(ExecutionPlan {
            capabilities: by_id.values().cloned().collect(),
            execution_order,
            missing_dependencies,
            parallel_groups,
        })
    }

    fn add_edge(
        from: &str,
        to: &str,
        adjacency: &mut HashMap<String, BTreeSet<String>>,
        indegree: &mut HashMap<String, usize>,
    ) {
        if adjacency
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string())
        {
            *indegree.entry(to.to_string()).or_insert(0) += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_topological_order_and_parallel_groups() {
        let manifests = vec![
            CapabilityManifest {
                id: "identity".to_string(),
                version: "v1".to_string(),
                provides: vec!["identity".to_string()],
                requires: vec![],
                before: vec![],
                after: vec![],
                tags: vec![],
                permissions: vec![],
                cost: 1,
            },
            CapabilityManifest {
                id: "policy".to_string(),
                version: "v1".to_string(),
                provides: vec!["policy".to_string()],
                requires: vec!["identity".to_string()],
                before: vec![],
                after: vec![],
                tags: vec![],
                permissions: vec![],
                cost: 1,
            },
            CapabilityManifest {
                id: "provider_router".to_string(),
                version: "v1".to_string(),
                provides: vec!["provider.selection".to_string()],
                requires: vec!["policy".to_string()],
                before: vec![],
                after: vec![],
                tags: vec![],
                permissions: vec![],
                cost: 1,
            },
        ];

        let plan = ExecutionPlanner::build_plan(manifests).expect("plan");
        assert_eq!(
            plan.execution_order,
            vec!["identity", "policy", "provider_router"]
        );
        assert_eq!(plan.parallel_groups.len(), 3);
    }
}
