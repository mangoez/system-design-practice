use std::{sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    middleware::{self, Next},
    response::IntoResponse,
    routing::get,
};
use consistent_hashing::virtual_node_ring::VirtualNodeRing;
use rate_limiter::{RateLimiterLayer, leaky_bucket::LeakyBucketRateLimiter};
use tokio::time::sleep;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{Level, info};

const SERVER_PORT: u16 = 8080;
const BACKENDS: [&str; 3] = ["server-a", "server-b", "server-c"];

async fn get_str() -> &'static str {
    sleep(Duration::from_millis(100)).await;
    "AH!"
}

/// Reports which backend owns a key, so the ring's routing can be inspected
/// without standing up the backends themselves.
async fn route_key(
    State(ring): State<Arc<VirtualNodeRing>>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    match ring.lookup(&key) {
        Some(server_id) => (StatusCode::OK, server_id.to_owned()),
        None => (StatusCode::SERVICE_UNAVAILABLE, "ring is empty".to_owned()),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Installs a global tracing subscriber that listens for events and writes them to stdout
    tracing_subscriber::fmt::fmt()
        // .with_span_events(FmtSpan::CLOSE)
        .with_max_level(Level::DEBUG)
        .init();

    let rate_limiter = Arc::new(RateLimiterLayer::new(LeakyBucketRateLimiter::new(5, 4)));

    // Membership is fixed at startup, so the ring never mutates and an Arc is
    // enough. Adding servers at runtime would need an RwLock around it.
    let ring = Arc::new(VirtualNodeRing::from_servers(BACKENDS));

    let app = Router::new()
        .route("/str", get(get_str))
        .route("/route/{key}", get(route_key))
        .with_state(ring)
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
