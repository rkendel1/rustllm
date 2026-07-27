use crate::{
    config::PolicyRule,
    kernel::{
        capability::{Capability, CapabilityFuture, CapabilityResult},
        context::RequestContext,
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

    fn on_request<'a>(&'a self, ctx: &'a mut RequestContext) -> CapabilityFuture<'a> {
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
                        return Ok(CapabilityResult::Deny {
                            message: format!("request denied by policy '{}'", rule.name),
                            kind: "policy_deny".to_string(),
                            status_code: 403,
                        });
                    }
                    if rule.require_approval {
                        ctx.policy.requires_approval = true;
                        return Ok(CapabilityResult::Deny {
                            message: format!("request requires approval by policy '{}'", rule.name),
                            kind: "policy_approval_required".to_string(),
                            status_code: 403,
                        });
                    }
                }
            }
            Ok(CapabilityResult::Continue)
        })
    }
}
