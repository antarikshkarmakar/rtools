// Clippy 1.95.0 ICEs in `RedundantClone::check_fn` while rendering this crate.
#![allow(clippy::redundant_clone)]

use axum::{
    response::IntoResponse,
    routing::{get, post, Router},
    Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;

mod handlers;

#[derive(Clone)]
struct AppState {
    config: rtools_core::AppConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load configuration
    let config = rtools_core::AppConfig::load(None)?;
    let state = AppState {
        config: config.clone(),
    };

    // Build CORS layer
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // Build router
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/v1/image/compress", post(handlers::image::compress))
        .route("/api/v1/image/convert", post(handlers::image::convert))
        .route("/api/v1/image/resize", post(handlers::image::resize))
        .route("/api/v1/image/crop", post(handlers::image::crop))
        .route("/api/v1/image/watermark", post(handlers::image::watermark))
        .route("/api/v1/image/filter", post(handlers::image::filter))
        .route("/api/v1/image/metadata", post(handlers::image::metadata))
        .route("/api/v1/pdf/merge", post(handlers::pdf::merge))
        .route("/api/v1/pdf/compress", post(handlers::pdf::compress))
        .route("/api/v1/pdf/split", post(handlers::pdf::split))
        .route("/api/v1/pdf/ocr", post(handlers::pdf::ocr))
        .route("/api/v1/ai/organize", post(handlers::ai::organize))
        .route("/api/v1/ai/rename", post(handlers::ai::rename))
        .route("/api/v1/ai/alt-text", post(handlers::ai::alt_text))
        .route("/api/v1/ai/duplicates", post(handlers::ai::duplicates))
        .layer(cors)
        .layer(RequestBodyLimitLayer::new(100 * 1024 * 1024)) // 100MB limit
        .with_state(Arc::new(state));

    // Start server
    let addr = SocketAddr::new(config.api.host.parse()?, config.api.port);

    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn root() -> &'static str {
    "rtools API - Image and PDF Processing Toolkit"
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

async fn health() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
