#[derive(Debug, Clone, Copy)]
pub enum LifecycleHook {
    OnRequest,
    OnResponse,
}

#[derive(Debug, Clone)]
pub struct LifecycleEvent {
    pub capability_id: String,
    pub hook: LifecycleHook,
}
