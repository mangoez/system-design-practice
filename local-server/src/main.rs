use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    http::StatusCode,
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
};
use rate_limiter::{RateLimiterLayer, leaky_bucket::LeakyBucketRateLimiter};
use tokio::time::sleep;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{Level, info};

const SERVER_PORT: u16 = 8080;

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

    let rate_limiter = Arc::new(RateLimiterLayer::new(LeakyBucketRateLimiter::new(5, 4)));

    let app = Router::new()
        .route("/str", get(get_str))
        .layer(middleware::from_fn(move |req, next: Next| {
            let limiter = rate_limiter.clone();
            async move {
                if !limiter.try_acquire() {
                    return StatusCode::TOO_MANY_REQUESTS.into_response();
                }
                next.run(req).await
            }
        }))
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", SERVER_PORT)).await?;
    info!(addr = %listener.local_addr()?, "listening");

    axum::serve(listener, app).await?;
    Ok(())
}
