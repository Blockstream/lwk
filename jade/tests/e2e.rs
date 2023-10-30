use std::sync::Once;

mod emulator;
mod pin_server;

#[cfg(feature = "serial")]
mod serial;

static TRACING_INIT: Once = Once::new();

fn init_logging() {
    use tracing_subscriber::prelude::*;

    TRACING_INIT.call_once(|| {
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(tracing_subscriber::EnvFilter::from_default_env())
            .init();

        tracing::info!("logging initialized");
    });
}
