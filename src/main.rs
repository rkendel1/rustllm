use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config_path = env::args()
        .nth(1)
        .or_else(|| env::var("AETHER_CONFIG").ok())
        .unwrap_or_else(|| "config.example.yaml".to_string());

    aether::run(&config_path).await
}
