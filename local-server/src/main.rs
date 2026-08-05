use std::time::Duration;

use axum::{Router, routing::get};
use tokio::time::sleep;
use tower_http::trace::TraceLayer;
use tower::ServiceBuilder;
use tracing::{Level, info, instrument};

const SERVER_PORT: u16 = 8080;

#[instrument]
async fn get_str() -> &'static str {
    sleep(Duration::from_millis(100)).await;
    "AH!"
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Installs a global tracing subscriber that listens for events and writes them to stdout
    tracing_subscriber::fmt::fmt()
        // .with_span_events(FmtSpan::CLOSE)
        .with_max_level(Level::DEBUG)
        .init();

    let app = Router::new()
        .route("/str", get(get_str))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
        );

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", SERVER_PORT)).await?;
    info!(addr = %listener.local_addr()?, "listening");
    
    axum::serve(listener, app).await?;
    Ok(())
}
