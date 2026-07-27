use crate::{
    config::PolicyRule,
    kernel::{
        capability::{Capability, CapabilityFuture, CapabilityResult},
        context::{CapabilityState, RequestContext},
        manifest::CapabilityManifest,
    },
};

#[derive(Clone)]
pub struct PolicyCapability {
    rules: Vec<PolicyRule>,
}

impl PolicyCapability {
    pub fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }
}

impl Capability for PolicyCapability {
    fn id(&self) -> &'static str {
        "policy"
    }

    fn version(&self) -> &'static str {
        "v1"
    }

    fn manifest(&self) -> CapabilityManifest {
        CapabilityManifest {
            id: self.id().to_string(),
            version: self.version().to_string(),
            provides: vec!["policy".to_string()],
            requires: vec!["identity".to_string()],
            before: vec![],
            after: vec![],
            tags: vec!["policy".to_string()],
            permissions: vec!["policy.enforce".to_string()],
            cost: 1,
        }
    }

    fn on_request<'a>(
        &'a self,
        ctx: &'a mut RequestContext,
        state: &'a mut CapabilityState,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            for rule in &self.rules {
                let applies_to_free_plan = rule
                    .when
                    .get("user.plan")
                    .map(|v| v == "free")
                    .unwrap_or(false);
                if applies_to_free_plan && ctx.identity.plan == "free" {
                    if rule
                        .deny
                        .models
                        .iter()
                        .any(|model| model == &ctx.model.model)
                    {
                        ctx.policy.denied_by = Some(rule.name.clone());
                        ctx.policy.matched_rules.push(rule.name.clone());
                        state.policy.matched_rules.push(rule.name.clone());
                        state.facts.publish(
                            "policy.matched_rules",
                            serde_json::json!(state.policy.matched_rules),
                        );
                        return Ok(CapabilityResult::Fail {
                            message: format!("request denied by policy '{}'", rule.name),
                            kind: "policy_deny".to_string(),
                            status_code: 403,
                        });
                    }
                    if rule.require_approval {
                        ctx.policy.requires_approval = true;
                        ctx.policy.approval_required = true;
                        state.policy.approval_required = true;
                        state
                            .facts
                            .publish("policy.approval_required", serde_json::json!(true));
                        return Ok(CapabilityResult::RequireApproval {
                            message: format!("request requires approval by policy '{}'", rule.name),
                            kind: "policy_approval_required".to_string(),
                            status_code: 403,
                        });
                    }
                }
            }
            state
                .facts
                .publish("policy.approval_required", serde_json::json!(false));
            Ok(CapabilityResult::Continue)
        })
    }
}
