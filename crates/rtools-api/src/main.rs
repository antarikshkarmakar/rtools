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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use image::ImageFormat;
    use std::collections::BTreeSet;
    use std::io::Cursor;
    use std::path::PathBuf;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    static IMAGE_ARTIFACT_TEST_LOCK: Mutex<()> = Mutex::const_new(());

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("test PNG must encode");
        bytes.into_inner()
    }

    fn multipart_request(path: &str, file: Option<(&str, Vec<u8>)>) -> Request<Body> {
        let boundary = "rtools-test-boundary";
        let mut body = Vec::new();
        if let Some((name, data)) = file {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
            body.extend_from_slice(&data);
            body.extend_from_slice(b"\r\n");
        }
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
        Request::builder()
            .method("POST")
            .uri(path)
            .header(
                "content-type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(Body::from(body))
            .expect("request must build")
    }

    fn test_app() -> Router {
        Router::new()
            .route("/compress", post(handlers::image::compress))
            .route("/convert", post(handlers::image::convert))
            .route("/resize", post(handlers::image::resize))
            .route("/rename", post(handlers::ai::rename))
            .route("/organize", post(handlers::ai::organize))
            .route("/duplicates", post(handlers::ai::duplicates))
            .with_state(Arc::new(AppState {
                config: rtools_core::AppConfig::default(),
            }))
    }

    fn persistent_image_temp_dirs() -> BTreeSet<PathBuf> {
        std::fs::read_dir(std::env::temp_dir())
            .expect("system temp directory must be readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rtools-api-image-"))
            })
            .collect()
    }

    #[tokio::test]
    async fn image_adapter_returns_live_sanitized_artifacts() {
        let _guard = IMAGE_ARTIFACT_TEST_LOCK.lock().await;
        for path in ["/compress", "/convert", "/resize"] {
            let response = test_app()
                .oneshot(multipart_request(
                    path,
                    Some(("..\\private\\input.png", png_bytes())),
                ))
                .await
                .expect("router call must complete");
            let status = response.status();
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("response body must read");
            assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
            let document: serde_json::Value =
                serde_json::from_slice(&body).expect("response must be JSON");
            let output_path = document["output_path"]
                .as_str()
                .map(std::path::PathBuf::from)
                .expect("response must include an output path");
            assert!(output_path.exists(), "{}", output_path.display());
            assert!(
                image::open(&output_path).is_ok(),
                "{}",
                output_path.display()
            );
            let artifact_dir = output_path.parent().expect("artifact parent");
            assert!(
                artifact_dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rtools-api-image-")),
                "{}",
                artifact_dir.display()
            );
            assert!(!output_path.display().to_string().contains("private"));
            std::fs::remove_dir_all(artifact_dir).expect("test artifact cleanup");
        }
    }

    #[tokio::test]
    async fn failed_image_processing_cleans_its_request_directory() {
        let _guard = IMAGE_ARTIFACT_TEST_LOCK.lock().await;
        let before = persistent_image_temp_dirs();

        let response = test_app()
            .oneshot(multipart_request(
                "/compress",
                Some(("invalid.png", b"not an image".to_vec())),
            ))
            .await
            .expect("router call must complete");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(persistent_image_temp_dirs(), before);
    }

    #[tokio::test]
    async fn ai_adapter_rejects_empty_duplicates_and_uses_deterministic_rename() {
        let organize_response = test_app()
            .oneshot(multipart_request("/organize", None))
            .await
            .expect("router call must complete");
        assert_eq!(
            organize_response.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );

        let duplicate_response = test_app()
            .oneshot(multipart_request("/duplicates", None))
            .await
            .expect("router call must complete");
        assert_eq!(
            duplicate_response.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        let duplicate_body = to_bytes(duplicate_response.into_body(), usize::MAX)
            .await
            .expect("response body must read");
        let duplicate_text = String::from_utf8_lossy(&duplicate_body);
        assert!(
            duplicate_text
                .to_ascii_lowercase()
                .contains("invalid input"),
            "{duplicate_text}"
        );

        let rename_response = test_app()
            .oneshot(multipart_request(
                "/rename",
                Some(("input.png", png_bytes())),
            ))
            .await
            .expect("router call must complete");
        let status = rename_response.status();
        let rename_body = to_bytes(rename_response.into_body(), usize::MAX)
            .await
            .expect("response body must read");
        let text = String::from_utf8_lossy(&rename_body);
        assert_eq!(status, StatusCode::OK, "{text}");
        assert!(!text.contains("{subject}"), "{text}");
    }
}
