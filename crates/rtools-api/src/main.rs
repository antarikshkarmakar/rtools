// Clippy 1.95.0 ICEs in `RedundantClone::check_fn` while rendering this crate.
#![allow(clippy::redundant_clone)]

use axum::{
    extract::DefaultBodyLimit,
    response::IntoResponse,
    routing::{get, post, Router},
    Json,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

mod handlers;

struct AppState {
    config: rtools_core::AppConfig,
    artifacts: ArtifactStore,
}

struct ArtifactStore {
    root: tempfile::TempDir,
}

impl AppState {
    fn new(config: rtools_core::AppConfig) -> anyhow::Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            artifacts: ArtifactStore {
                root: tempfile::Builder::new()
                    .prefix("rtools-api-artifacts-")
                    .tempdir()?,
            },
        })
    }
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
    let app = build_router(config.clone())?;

    // Start server
    let addr = SocketAddr::new(config.api.host.parse()?, config.api.port);

    tracing::info!("Starting server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn build_router(config: rtools_core::AppConfig) -> anyhow::Result<Router> {
    let upload_limit = usize::try_from(config.api.max_upload_size).map_err(|_| {
        anyhow::anyhow!("api.max_upload_size exceeds this platform's addressable size")
    })?;
    let state = Arc::new(AppState::new(config)?);
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Ok(Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/api/v1/artifacts/:id", get(handlers::artifact::download))
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
        .layer(DefaultBodyLimit::max(upload_limit))
        .with_state(state))
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
mod package_b_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use base64::Engine as _;
    use image::ImageFormat;
    use std::collections::BTreeSet;
    use std::io::Cursor;
    use std::path::PathBuf;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    static IMAGE_ARTIFACT_TEST_LOCK: Mutex<()> = Mutex::const_new(());

    fn image_bytes(format: ImageFormat) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::new_rgba8(2, 2)
            .write_to(&mut bytes, format)
            .expect("test image must encode");
        bytes.into_inner()
    }

    fn png_bytes() -> Vec<u8> {
        image_bytes(ImageFormat::Png)
    }

    fn orientation_jpeg_bytes() -> Vec<u8> {
        base64::engine::general_purpose::STANDARD
            .decode(include_str!("../../rtools-tests/fixtures/images/orientation-6.jpg.b64").trim())
            .expect("orientation fixture must decode")
    }

    fn multipart_request(path: &str, file: Option<(&str, Vec<u8>)>) -> Request<Body> {
        let boundary = "rtools-test-boundary";
        let mut body = Vec::new();
        if let Some((name, data)) = file {
            let field = if matches!(path, "/rename" | "/organize" | "/duplicates") {
                "files"
            } else {
                "file"
            };
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{name}\"\r\nContent-Type: image/png\r\n\r\n").as_bytes());
            body.extend_from_slice(&data);
            body.extend_from_slice(b"\r\n");
        }
        if path == "/convert" {
            body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"format\"\r\n\r\nwebp\r\n").as_bytes());
        }
        if path == "/resize" {
            body.extend_from_slice(
                format!(
                    "--{boundary}\r\nContent-Disposition: form-data; name=\"width\"\r\n\r\n800\r\n"
                )
                .as_bytes(),
            );
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
            .route("/api/v1/artifacts/:id", get(handlers::artifact::download))
            .route("/compress", post(handlers::image::compress))
            .route("/convert", post(handlers::image::convert))
            .route("/resize", post(handlers::image::resize))
            .route("/metadata", post(handlers::image::metadata))
            .route("/rename", post(handlers::ai::rename))
            .route("/organize", post(handlers::ai::organize))
            .route("/duplicates", post(handlers::ai::duplicates))
            .route("/api/v1/ai/alt-text", post(handlers::ai::alt_text))
            .with_state(Arc::new(
                AppState::new(rtools_core::AppConfig::default()).unwrap(),
            ))
    }

    fn persistent_request_temp_dirs() -> BTreeSet<PathBuf> {
        std::fs::read_dir(std::env::temp_dir())
            .expect("system temp directory must be readable")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("rtools-api-request-"))
            })
            .collect()
    }

    #[tokio::test]
    async fn image_adapter_returns_live_sanitized_artifacts() {
        let _guard = IMAGE_ARTIFACT_TEST_LOCK.lock().await;
        let _request_guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        for path in ["/compress", "/convert", "/resize"] {
            let app = test_app();
            let response = app
                .clone()
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
            assert!(document.get("warnings").is_none(), "{path}: {document}");
            assert!(document.get("output_path").is_none(), "{path}: {document}");
            let download_url = document["artifact"]["download_url"]
                .as_str()
                .expect("response must include a download URL");
            let artifact = app
                .oneshot(
                    Request::builder()
                        .uri(download_url)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(artifact.status(), StatusCode::OK);
            let bytes = to_bytes(artifact.into_body(), usize::MAX).await.unwrap();
            assert!(image::load_from_memory(&bytes).is_ok());
            assert!(!document.to_string().contains("private"));
        }
    }

    #[test]
    fn image_response_warnings_are_backward_compatible_and_skip_empty_serialization() {
        let compress: handlers::image::CompressResponse =
            serde_json::from_value(serde_json::json!({
                "success": true,
                "message": "ok",
                "artifact": {
                    "id": "artifact-example.png",
                    "download_url": "/api/v1/artifacts/artifact-example.png",
                    "name": "example.png",
                    "media_type": "image/png"
                },
                "stats": null
            }))
            .unwrap();
        assert!(compress.warnings.is_empty());
        assert!(serde_json::to_value(compress)
            .unwrap()
            .get("warnings")
            .is_none());

        let convert: handlers::image::ConvertResponse = serde_json::from_value(serde_json::json!({
            "success": true,
            "message": "ok",
            "artifact": {
                "id": "artifact-example.png",
                "download_url": "/api/v1/artifacts/artifact-example.png",
                "name": "example.png",
                "media_type": "image/png"
            }
        }))
        .unwrap();
        assert!(convert.warnings.is_empty());
        assert!(serde_json::to_value(convert)
            .unwrap()
            .get("warnings")
            .is_none());
    }

    #[tokio::test]
    async fn image_adapter_returns_orientation_warnings_and_oriented_live_artifacts() {
        let _guard = IMAGE_ARTIFACT_TEST_LOCK.lock().await;
        let _request_guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        for (path, expected_dimensions) in [
            ("/compress", (36, 24)),
            ("/convert", (36, 24)),
            ("/resize", (800, 533)),
        ] {
            let app = test_app();
            let response = app
                .clone()
                .oneshot(multipart_request(
                    path,
                    Some(("orientation.jpg", orientation_jpeg_bytes())),
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
            assert_eq!(
                document["warnings"],
                serde_json::json!(["EXIF orientation 6 applied"]),
                "{path}: {}",
                String::from_utf8_lossy(&body)
            );
            let download_url = document["artifact"]["download_url"]
                .as_str()
                .expect("response must include a download URL");
            let artifact = app
                .oneshot(
                    Request::builder()
                        .uri(download_url)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(artifact.status(), StatusCode::OK);
            let bytes = to_bytes(artifact.into_body(), usize::MAX).await.unwrap();
            let image = image::load_from_memory(&bytes).unwrap();
            assert_eq!((image.width(), image.height()), expected_dimensions);
        }
    }

    #[tokio::test]
    async fn failed_image_processing_cleans_its_request_directory() {
        let _guard = IMAGE_ARTIFACT_TEST_LOCK.lock().await;
        let _request_guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        let before = persistent_request_temp_dirs();

        let response = test_app()
            .oneshot(multipart_request(
                "/compress",
                Some(("invalid.png", b"not an image".to_vec())),
            ))
            .await
            .expect("router call must complete");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(persistent_request_temp_dirs(), before);
    }

    #[tokio::test]
    async fn metadata_adapter_treats_valid_bmp_and_gif_as_empty_exif() {
        let _guard = IMAGE_ARTIFACT_TEST_LOCK.lock().await;
        let _request_guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        for (name, format) in [
            ("plain.bmp", ImageFormat::Bmp),
            ("plain.gif", ImageFormat::Gif),
        ] {
            let response = test_app()
                .oneshot(multipart_request(
                    "/metadata",
                    Some((name, image_bytes(format))),
                ))
                .await
                .expect("router call must complete");
            let status = response.status();
            let body = to_bytes(response.into_body(), usize::MAX)
                .await
                .expect("response body must read");
            assert_eq!(
                status,
                StatusCode::OK,
                "{name}: {}",
                String::from_utf8_lossy(&body)
            );
            let document: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(document["success"], true, "{name}: {document}");
            assert!(
                document["metadata"]["exif"]
                    .as_object()
                    .unwrap()
                    .values()
                    .all(serde_json::Value::is_null),
                "{name}: {document}"
            );
        }
    }

    #[tokio::test]
    async fn ai_adapter_rejects_empty_duplicates_and_uses_deterministic_rename() {
        let _guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        let organize_response = test_app()
            .oneshot(multipart_request("/organize", None))
            .await
            .expect("router call must complete");
        assert_eq!(organize_response.status(), StatusCode::BAD_REQUEST);

        let duplicate_response = test_app()
            .oneshot(multipart_request("/duplicates", None))
            .await
            .expect("router call must complete");
        assert_eq!(duplicate_response.status(), StatusCode::BAD_REQUEST);
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

    #[tokio::test]
    async fn duplicate_adapter_uses_configured_decoded_pixel_limit() {
        let _guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        let mut config = rtools_core::AppConfig::default();
        config.limits.max_decoded_pixels = 1;
        let app = Router::new()
            .route("/duplicates", post(handlers::ai::duplicates))
            .with_state(Arc::new(AppState::new(config).unwrap()));

        let response = app
            .oneshot(multipart_request(
                "/duplicates",
                Some(("declared-canvas.png", png_bytes())),
            ))
            .await
            .expect("router call must complete");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body must read");
        let text = String::from_utf8_lossy(&body);

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{text}");
        assert!(text.contains("decoded_pixels"), "{text}");
        assert!(text.contains("4 (limit: 1)"), "{text}");
    }

    #[tokio::test]
    async fn empty_alt_text_request_returns_structured_invalid_input() {
        let _guard = package_b_tests::REQUEST_DIRECTORY_TEST_LOCK.lock().await;
        let before = persistent_request_temp_dirs();
        let response = test_app()
            .oneshot(multipart_request("/api/v1/ai/alt-text", None))
            .await
            .expect("router call must complete");
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body must read");
        let document: serde_json::Value =
            serde_json::from_slice(&body).expect("error response must be JSON");

        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(document["success"], false);
        assert_eq!(document["code"], "CAPABILITY_UNAVAILABLE");
        assert!(document.get("results").is_none());
        assert_eq!(persistent_request_temp_dirs(), before);
    }
}
