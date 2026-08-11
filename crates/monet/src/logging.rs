use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn enable_tracing() {
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "monet=trace".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .try_init();
}
