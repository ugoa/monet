use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, util::SubscriberInitExt};

pub fn enable_tracing() {
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::TRACE.into())
        .from_env_lossy();

    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .finish()
        .try_init();
}
