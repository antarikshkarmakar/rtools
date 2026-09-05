use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{header, header::CONTENT_TYPE, Method, Request, StatusCode},
};
use futures_util::StreamExt as _;
use image::ImageFormat;
use lopdf::{dictionary, Document, Object, Stream};
use serde_json::Value;
use std::collections::BTreeSet;
use std::io::Cursor;
use std::path::PathBuf;
use tower::ServiceExt;

pub static REQUEST_DIRECTORY_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

enum Part<'a> {
    File {
        field: &'a str,
        name: &'a str,
        content_type: &'a str,
        bytes: Vec<u8>,
    },
    Text {
        field: &'a str,
        value: &'a str,
    },
    RawText {
        field: &'a str,
        bytes: Vec<u8>,
    },
}

fn multipart(path: &str, parts: Vec<Part<'_>>) -> Request<Body> {
    const BOUNDARY: &str = "rtools-package-b-boundary";
    let mut body = Vec::new();
    for part in parts {
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        match part {
            Part::File {
                field,
                name,
                content_type,
                bytes,
            } => {
                body.extend_from_slice(
                    format!(
                        "Content-Disposition: form-data; name=\"{field}\"; filename=\"{name}\"\r\nContent-Type: {content_type}\r\n\r\n"
                    )
                    .as_bytes(),
                );
                body.extend_from_slice(&bytes);
                body.extend_from_slice(b"\r\n");
            }
            Part::Text { field, value } => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{field}\"\r\n\r\n{value}\r\n")
                        .as_bytes(),
                );
            }
            Part::RawText { field, bytes } => {
                body.extend_from_slice(
                    format!("Content-Disposition: form-data; name=\"{field}\"\r\n\r\n").as_bytes(),
                );
                body.extend_from_slice(&bytes);
                body.extend_from_slice(b"\r\n");
            }
        }
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    Request::builder()
        .method("POST")
        .uri(path)
        .header(
            CONTENT_TYPE,
            format!("multipart/form-data; boundary={BOUNDARY}"),
        )
        .body(Body::from(body))
        .unwrap()
}

fn png_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, ImageFormat::Png)
        .unwrap();
    bytes.into_inner()
}

fn jpeg_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(width, height)
        .write_to(&mut bytes, ImageFormat::Jpeg)
        .unwrap();
    bytes.into_inner()
}

fn pdf_bytes() -> Vec<u8> {
    let mut document = Document::with_version("1.5");
    let pages_id = document.new_object_id();
    let catalog_id = document.new_object_id();
    let content_id = document.add_object(Stream::new(dictionary! {}, Vec::new()));
    let leaf_page_id = document.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 100.into(), 100.into()],
        "Contents" => content_id,
    });
    document.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => vec![Object::Reference(leaf_page_id)],
            "Count" => 1,
        }),
    );
    document.objects.insert(
        catalog_id,
        Object::Dictionary(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        }),
    );
    document.trailer.set("Root", catalog_id);
    let mut bytes = Vec::new();
    document.save_to(&mut bytes).unwrap();
    bytes
}

fn app() -> Router {
    build_router(rtools_core::AppConfig::default()).unwrap()
}

async fn json_response(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let document = serde_json::from_slice(&body)
        .unwrap_or_else(|_| panic!("response was not JSON: {}", String::from_utf8_lossy(&body)));
    (status, document)
}

fn request_directories() -> BTreeSet<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("rtools-api-request-"))
        })
        .collect()
}

async fn download(app: &Router, document: &Value) -> Vec<u8> {
    let url = document["artifact"]["download_url"].as_str().unwrap();
    assert!(url.starts_with("/api/v1/artifacts/"));
    assert!(!url.contains('\\'));
    assert!(!url.contains(".."));
    let response = app
        .clone()
        .oneshot(Request::builder().uri(url).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

fn assert_structured(status: StatusCode, document: &Value, expected: StatusCode) {
    assert_eq!(status, expected, "{document}");
    assert_eq!(document["success"], false, "{document}");
    assert!(document["code"].is_string(), "{document}");
    assert!(document["message"].is_string(), "{document}");
}

#[tokio::test]
async fn image_upload_name_is_never_used_as_a_storage_path_and_artifact_is_downloadable() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let sandbox = tempfile::tempdir().unwrap();
    let sentinel = sandbox.path().join("outside.png");
    std::fs::write(&sentinel, b"sentinel").unwrap();
    let raw_name = sentinel.to_string_lossy().replace('\\', "/");
    let app = app();

    let (status, document) = json_response(
        &app,
        multipart(
            "/api/v1/image/compress",
            vec![Part::File {
                field: "file",
                name: &raw_name,
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            }],
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{document}");
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");
    assert!(document.get("output_path").is_none(), "{document}");
    assert!(!document
        .to_string()
        .contains(sandbox.path().to_string_lossy().as_ref()));
    assert!(image::load_from_memory(&download(&app, &document).await).is_ok());
}

async fn assert_raw_names_ignored<'a, const N: usize>(
    sandbox: &std::path::Path,
    image_sentinel: &std::path::Path,
    pdf_sentinel: &std::path::Path,
    cases: [(&'a str, Vec<Part<'a>>); N],
) {
    for (path, parts) in cases {
        let (status, document) = json_response(&app(), multipart(path, parts)).await;
        assert_eq!(status, StatusCode::OK, "{path}: {document}");
        assert!(!document
            .to_string()
            .contains(sandbox.to_string_lossy().as_ref()));
        assert_eq!(std::fs::read(image_sentinel).unwrap(), b"image sentinel");
        assert_eq!(std::fs::read(pdf_sentinel).unwrap(), b"pdf sentinel");
    }
}

fn raw_name_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, String, String) {
    let sandbox = tempfile::tempdir().unwrap();
    let image_sentinel = sandbox.path().join("outside.png");
    let pdf_sentinel = sandbox.path().join("outside.pdf");
    std::fs::write(&image_sentinel, b"image sentinel").unwrap();
    std::fs::write(&pdf_sentinel, b"pdf sentinel").unwrap();
    let image_name = image_sentinel.to_string_lossy().replace('\\', "/");
    let pdf_name = pdf_sentinel.to_string_lossy().replace('\\', "/");
    (sandbox, image_sentinel, pdf_sentinel, image_name, pdf_name)
}

#[tokio::test]
async fn image_upload_adapters_ignore_raw_filename_paths() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let (sandbox, image_sentinel, pdf_sentinel, image_name, _) = raw_name_fixture();
    let image = png_bytes(2, 2);
    assert_raw_names_ignored(
        sandbox.path(),
        &image_sentinel,
        &pdf_sentinel,
        [
            (
                "/api/v1/image/convert",
                vec![
                    Part::File {
                        field: "file",
                        name: &image_name,
                        content_type: "image/png",
                        bytes: image.clone(),
                    },
                    Part::Text {
                        field: "format",
                        value: "webp",
                    },
                ],
            ),
            (
                "/api/v1/image/resize",
                vec![
                    Part::File {
                        field: "file",
                        name: &image_name,
                        content_type: "image/png",
                        bytes: image.clone(),
                    },
                    Part::Text {
                        field: "width",
                        value: "1",
                    },
                ],
            ),
            (
                "/api/v1/image/metadata",
                vec![Part::File {
                    field: "file",
                    name: &image_name,
                    content_type: "image/png",
                    bytes: image.clone(),
                }],
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn ai_and_pdf_upload_adapters_ignore_raw_filename_paths() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let (sandbox, image_sentinel, pdf_sentinel, image_name, pdf_name) = raw_name_fixture();
    let image = png_bytes(2, 2);
    let pdf = pdf_bytes();
    assert_raw_names_ignored(
        sandbox.path(),
        &image_sentinel,
        &pdf_sentinel,
        [
            (
                "/api/v1/ai/organize",
                vec![Part::File {
                    field: "files",
                    name: &image_name,
                    content_type: "image/png",
                    bytes: image.clone(),
                }],
            ),
            (
                "/api/v1/ai/rename",
                vec![
                    Part::File {
                        field: "files",
                        name: &image_name,
                        content_type: "image/png",
                        bytes: image.clone(),
                    },
                    Part::Text {
                        field: "pattern",
                        value: "renamed_{index}",
                    },
                ],
            ),
            (
                "/api/v1/ai/duplicates",
                vec![Part::File {
                    field: "files",
                    name: &image_name,
                    content_type: "image/png",
                    bytes: image,
                }],
            ),
            (
                "/api/v1/pdf/compress",
                vec![Part::File {
                    field: "file",
                    name: &pdf_name,
                    content_type: "application/pdf",
                    bytes: pdf.clone(),
                }],
            ),
            (
                "/api/v1/pdf/merge",
                vec![
                    Part::File {
                        field: "files",
                        name: &pdf_name,
                        content_type: "application/pdf",
                        bytes: pdf.clone(),
                    },
                    Part::File {
                        field: "files",
                        name: &pdf_name,
                        content_type: "application/pdf",
                        bytes: pdf,
                    },
                ],
            ),
        ],
    )
    .await;
}

#[tokio::test]
async fn resize_honors_named_parameters_independent_of_part_order() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let app = app();
    let (status, document) = json_response(
        &app,
        multipart(
            "/api/v1/image/resize",
            vec![
                Part::Text {
                    field: "width",
                    value: "3",
                },
                Part::File {
                    field: "file",
                    name: "photo.png",
                    content_type: "image/png",
                    bytes: png_bytes(8, 4),
                },
                Part::Text {
                    field: "maintain_aspect",
                    value: "true",
                },
            ],
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{document}");
    let output = image::load_from_memory(&download(&app, &document).await).unwrap();
    assert_eq!((output.width(), output.height()), (3, 2));
}

#[tokio::test]
async fn multipart_contract_rejects_unknown_duplicate_and_mistyped_fields_as_structured_400() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let cases = [
        vec![
            Part::File {
                field: "file",
                name: "photo.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
            Part::Text {
                field: "mystery",
                value: "1",
            },
        ],
        vec![
            Part::File {
                field: "file",
                name: "photo.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
            Part::Text {
                field: "width",
                value: "2",
            },
            Part::Text {
                field: "width",
                value: "3",
            },
        ],
        vec![
            Part::File {
                field: "file",
                name: "photo.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
            Part::Text {
                field: "width",
                value: "NaN",
            },
        ],
        vec![
            Part::File {
                field: "file",
                name: "first.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
            Part::File {
                field: "file",
                name: "second.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
        ],
        vec![Part::Text {
            field: "file",
            value: "not-a-file",
        }],
        vec![
            Part::File {
                field: "file",
                name: "photo.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
            Part::File {
                field: "width",
                name: "not-text.txt",
                content_type: "text/plain",
                bytes: b"2".to_vec(),
            },
        ],
        vec![Part::File {
            field: "file",
            name: "",
            content_type: "image/png",
            bytes: png_bytes(2, 2),
        }],
    ];

    for parts in cases {
        let (status, document) =
            json_response(&app(), multipart("/api/v1/image/resize", parts)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{document}");
        assert_eq!(document["success"], false, "{document}");
        assert_eq!(document["code"], "INVALID_INPUT", "{document}");
    }
}

#[tokio::test]
async fn implemented_endpoints_reject_unknown_or_missing_named_fields() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let image = png_bytes(2, 2);
    let pdf = pdf_bytes();
    let cases = [
        (
            "/api/v1/image/compress",
            vec![
                Part::File {
                    field: "file",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::Text {
                    field: "unknown",
                    value: "value",
                },
            ],
        ),
        (
            "/api/v1/image/convert",
            vec![Part::File {
                field: "file",
                name: "input.png",
                content_type: "image/png",
                bytes: image.clone(),
            }],
        ),
        (
            "/api/v1/image/resize",
            vec![Part::Text {
                field: "width",
                value: "2",
            }],
        ),
        (
            "/api/v1/image/metadata",
            vec![Part::Text {
                field: "include_exif",
                value: "true",
            }],
        ),
        (
            "/api/v1/pdf/merge",
            vec![Part::File {
                field: "files",
                name: "one.pdf",
                content_type: "application/pdf",
                bytes: pdf.clone(),
            }],
        ),
        (
            "/api/v1/pdf/compress",
            vec![Part::Text {
                field: "level",
                value: "medium",
            }],
        ),
        (
            "/api/v1/ai/organize",
            vec![Part::Text {
                field: "strategy",
                value: "date",
            }],
        ),
        (
            "/api/v1/ai/rename",
            vec![Part::Text {
                field: "pattern",
                value: "renamed_{index}",
            }],
        ),
        (
            "/api/v1/ai/duplicates",
            vec![Part::Text {
                field: "threshold",
                value: "0.9",
            }],
        ),
    ];

    for (path, parts) in cases {
        let (status, document) = json_response(&app(), multipart(path, parts)).await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn multipart_text_types_numbers_and_booleans_are_strict() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let image = png_bytes(2, 2);
    let invalid_cases = [
        (
            "/api/v1/image/resize",
            vec![
                Part::File {
                    field: "file",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::RawText {
                    field: "width",
                    bytes: vec![0xff],
                },
            ],
        ),
        (
            "/api/v1/image/compress",
            vec![
                Part::File {
                    field: "file",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::Text {
                    field: "preserve_metadata",
                    value: "yes",
                },
            ],
        ),
        (
            "/api/v1/image/convert",
            vec![
                Part::File {
                    field: "file",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::Text {
                    field: "format",
                    value: "not-an-image-format",
                },
            ],
        ),
        (
            "/api/v1/ai/duplicates",
            vec![
                Part::File {
                    field: "files",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::Text {
                    field: "threshold",
                    value: "NaN",
                },
            ],
        ),
    ];

    for (path, parts) in invalid_cases {
        let (status, document) = json_response(&app(), multipart(path, parts)).await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn ai_and_pdf_enum_values_are_strict() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let image = png_bytes(2, 2);
    let pdf = pdf_bytes();
    let invalid_cases = [
        (
            "/api/v1/ai/duplicates",
            vec![
                Part::File {
                    field: "files",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::Text {
                    field: "algorithm",
                    value: "unknown",
                },
            ],
        ),
        (
            "/api/v1/ai/organize",
            vec![
                Part::File {
                    field: "files",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image.clone(),
                },
                Part::Text {
                    field: "strategy",
                    value: "unknown",
                },
            ],
        ),
        (
            "/api/v1/pdf/compress",
            vec![
                Part::File {
                    field: "file",
                    name: "input.pdf",
                    content_type: "application/pdf",
                    bytes: pdf,
                },
                Part::Text {
                    field: "level",
                    value: "unknown",
                },
            ],
        ),
    ];

    for (path, parts) in invalid_cases {
        let (status, document) = json_response(&app(), multipart(path, parts)).await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn recognized_but_unavailable_options_return_structured_501() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let image = png_bytes(2, 2);
    let pdf = pdf_bytes();
    for (path, parts) in [
        (
            "/api/v1/ai/organize",
            vec![
                Part::File {
                    field: "files",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: image,
                },
                Part::Text {
                    field: "strategy",
                    value: "subject",
                },
            ],
        ),
        (
            "/api/v1/pdf/compress",
            vec![
                Part::File {
                    field: "file",
                    name: "input.pdf",
                    content_type: "application/pdf",
                    bytes: pdf,
                },
                Part::Text {
                    field: "level",
                    value: "heavy",
                },
            ],
        ),
    ] {
        let (status, document) = json_response(&app(), multipart(path, parts)).await;
        assert_structured(status, &document, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(document["code"], "CAPABILITY_UNAVAILABLE", "{document}");
    }
}

#[tokio::test]
async fn pdf_compress_uses_the_configured_default_level_fail_closed() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let mut config = rtools_core::AppConfig::default();
    config.pdf.compression_level = rtools_core::config::PdfCompressionLevel::Light;
    let app = build_router(config).unwrap();
    let (status, document) = json_response(
        &app,
        multipart(
            "/api/v1/pdf/compress",
            vec![Part::File {
                field: "file",
                name: "input.pdf",
                content_type: "application/pdf",
                bytes: pdf_bytes(),
            }],
        ),
    )
    .await;

    assert_structured(status, &document, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(document["code"], "CAPABILITY_UNAVAILABLE", "{document}");
}

#[tokio::test]
async fn unavailable_mode_is_rejected_before_any_request_directory_is_created() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let before = request_directories();
    let app = app();

    let (status, document) = json_response(
        &app,
        multipart(
            "/api/v1/ai/organize",
            vec![
                Part::File {
                    field: "files",
                    name: "photo.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
                Part::Text {
                    field: "strategy",
                    value: "subject",
                },
            ],
        ),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{document}");
    assert_eq!(document["code"], "CAPABILITY_UNAVAILABLE");
    assert_eq!(request_directories(), before);
}

#[tokio::test]
async fn reserved_and_unusual_client_names_never_become_artifact_paths() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for name in [
        "CON.png",
        "..\\escape.png",
        "C:\\escape.png",
        "odd<>|?*.png",
    ] {
        let app = app();
        let (status, document) = json_response(
            &app,
            multipart(
                "/api/v1/image/compress",
                vec![Part::File {
                    field: "file",
                    name,
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                }],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{name}: {document}");
        let id = document["artifact"]["id"].as_str().unwrap();
        assert!(handlers::artifact::valid_artifact_id(id), "{name}: {id}");
        assert_eq!(
            document["artifact"]["download_url"],
            format!("/api/v1/artifacts/{id}")
        );
        assert!(image::load_from_memory(&download(&app, &document).await).is_ok());
    }
}

#[tokio::test]
async fn ai_outputs_remain_downloadable_after_handler_return() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let app = app();

    let (organize_status, organize) = json_response(
        &app,
        multipart(
            "/api/v1/ai/organize",
            vec![Part::File {
                field: "files",
                name: "organized.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            }],
        ),
    )
    .await;
    assert_eq!(organize_status, StatusCode::OK, "{organize}");
    let organize_url = organize["results"]["artifacts"][0]["download_url"]
        .as_str()
        .unwrap();
    let organize_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(organize_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(organize_response.status(), StatusCode::OK);
    let organize_bytes = to_bytes(organize_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(image::load_from_memory(&organize_bytes).is_ok());

    let (rename_status, rename) = json_response(
        &app,
        multipart(
            "/api/v1/ai/rename",
            vec![
                Part::File {
                    field: "files",
                    name: "original.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
                Part::Text {
                    field: "pattern",
                    value: "renamed_{index}",
                },
            ],
        ),
    )
    .await;
    assert_eq!(rename_status, StatusCode::OK, "{rename}");
    let rename_url = rename["results"]["artifacts"][0]["download_url"]
        .as_str()
        .unwrap();
    let rename_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(rename_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rename_response.status(), StatusCode::OK);
    let rename_bytes = to_bytes(rename_response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert!(image::load_from_memory(&rename_bytes).is_ok());
}

#[tokio::test]
async fn pdf_outputs_remain_downloadable_after_handler_return() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let app = app();
    for (path, parts) in [
        (
            "/api/v1/pdf/compress",
            vec![Part::File {
                field: "file",
                name: "..\\outside.pdf",
                content_type: "application/pdf",
                bytes: pdf_bytes(),
            }],
        ),
        (
            "/api/v1/pdf/merge",
            vec![
                Part::File {
                    field: "files",
                    name: "same.pdf",
                    content_type: "application/pdf",
                    bytes: pdf_bytes(),
                },
                Part::File {
                    field: "files",
                    name: "same.pdf",
                    content_type: "application/pdf",
                    bytes: pdf_bytes(),
                },
            ],
        ),
    ] {
        let (status, document) = json_response(&app, multipart(path, parts)).await;
        assert_eq!(status, StatusCode::OK, "{path}: {document}");
        let bytes = download(&app, &document).await;
        assert!(bytes.starts_with(b"%PDF-"), "{path}");
    }
}

#[tokio::test]
async fn pdf_metadata_removal_is_unavailable_before_file_requirement() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let (status, document) = json_response(
        &app(),
        multipart(
            "/api/v1/pdf/compress",
            vec![Part::Text {
                field: "remove_metadata",
                value: "true",
            }],
        ),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{document}");
    assert_eq!(document["code"], "CAPABILITY_UNAVAILABLE");
    assert_eq!(document["details"]["operation_id"], "pdf.compress.metadata");
}

#[tokio::test]
async fn rename_pattern_cannot_select_a_path_outside_request_storage() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let sandbox = tempfile::tempdir().unwrap();
    let sentinel = sandbox.path().join("sentinel.png");
    std::fs::write(&sentinel, b"sentinel").unwrap();
    let pattern = format!("{}/escaped", sandbox.path().display());

    let (status, document) = json_response(
        &app(),
        multipart(
            "/api/v1/ai/rename",
            vec![
                Part::File {
                    field: "files",
                    name: "original.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
                Part::Text {
                    field: "pattern",
                    value: &pattern,
                },
            ],
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "{document}");
    assert_eq!(document["code"], "INVALID_INPUT");
    assert_eq!(std::fs::read(&sentinel).unwrap(), b"sentinel");
    assert!(!sandbox.path().join("escaped.png").exists());
}

#[tokio::test]
async fn rename_rejects_a_reserved_name_after_client_name_substitution() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let (status, document) = json_response(
        &app(),
        multipart(
            "/api/v1/ai/rename",
            vec![
                Part::File {
                    field: "files",
                    name: "CON.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
                Part::Text {
                    field: "pattern",
                    value: "{name}",
                },
            ],
        ),
    )
    .await;

    assert_structured(status, &document, StatusCode::BAD_REQUEST);
    assert_eq!(document["code"], "INVALID_INPUT", "{document}");
}

#[tokio::test]
async fn unavailable_endpoints_return_before_parsing_a_malformed_multipart_body() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for path in [
        "/api/v1/image/crop",
        "/api/v1/image/filter",
        "/api/v1/image/watermark",
        "/api/v1/pdf/split",
        "/api/v1/pdf/ocr",
        "/api/v1/ai/alt-text",
    ] {
        let request = Request::builder()
            .method("POST")
            .uri(path)
            .header(CONTENT_TYPE, "multipart/form-data; boundary=broken")
            .body(Body::from("definitely not multipart"))
            .unwrap();
        let (status, document) = json_response(&app(), request).await;
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED, "{path}: {document}");
        assert_eq!(document["code"], "CAPABILITY_UNAVAILABLE", "{path}");
    }
}

#[tokio::test]
async fn implemented_endpoints_wrap_missing_or_invalid_multipart_boundaries_as_json() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for content_type in [None, Some("multipart/form-data; boundary=broken")] {
        let mut request = Request::builder()
            .method("POST")
            .uri("/api/v1/image/compress");
        if let Some(content_type) = content_type {
            request = request.header(CONTENT_TYPE, content_type);
        }
        let response = app()
            .oneshot(request.body(Body::from("not multipart")).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            response.headers()[CONTENT_TYPE],
            "application/json",
            "multipart extractor rejections must use the API error envelope"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let document: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(document["success"], false);
        assert_eq!(document["code"], "INVALID_INPUT");
    }
}

#[tokio::test]
async fn incompatible_metadata_flags_are_invalid_before_capability_selection() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for path in ["/api/v1/image/compress", "/api/v1/image/convert"] {
        for target in ["avif", "ico"] {
            for flags_first in [true, false] {
                let parts = if flags_first {
                    vec![
                        Part::Text {
                            field: "preserve_metadata",
                            value: "true",
                        },
                        Part::Text {
                            field: "strip_gps",
                            value: "true",
                        },
                        Part::Text {
                            field: "format",
                            value: target,
                        },
                        Part::File {
                            field: "file",
                            name: "input.png",
                            content_type: "image/png",
                            bytes: png_bytes(2, 2),
                        },
                    ]
                } else {
                    vec![
                        Part::Text {
                            field: "format",
                            value: target,
                        },
                        Part::File {
                            field: "file",
                            name: "input.png",
                            content_type: "image/png",
                            bytes: png_bytes(2, 2),
                        },
                        Part::Text {
                            field: "strip_gps",
                            value: "true",
                        },
                        Part::Text {
                            field: "preserve_metadata",
                            value: "true",
                        },
                    ]
                };
                let before = request_directories();
                let (status, document) = json_response(&app(), multipart(path, parts)).await;
                assert_structured(status, &document, StatusCode::BAD_REQUEST);
                assert_eq!(document["code"], "INVALID_INPUT");
                assert_eq!(request_directories(), before);
            }
        }
    }
}

#[tokio::test]
async fn ai_rejects_declared_extension_and_encoded_format_mismatches() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for path in [
        "/api/v1/ai/organize",
        "/api/v1/ai/rename",
        "/api/v1/ai/duplicates",
    ] {
        for name in ["spoof.png", "spoof.heic", "spoof.unknown"] {
            let before = request_directories();
            let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
            let router = build_router_with_state(state.clone(), 1024 * 1024);
            let (status, document) = json_response(
                &router,
                multipart(
                    path,
                    vec![Part::File {
                        field: "files",
                        name,
                        content_type: "image/png",
                        bytes: jpeg_bytes(4, 4),
                    }],
                ),
            )
            .await;
            assert_structured(status, &document, StatusCode::BAD_REQUEST);
            let serialized = document.to_string();
            assert!(!serialized.contains("rtools-api-request-"));
            assert!(state.artifacts.records.read().await.is_empty());
            assert_eq!(
                std::fs::read_dir(state.artifacts.root.path())
                    .unwrap()
                    .count(),
                0
            );
            assert_eq!(request_directories(), before);
        }
    }
}

#[tokio::test]
async fn framework_routing_rejections_use_the_structured_json_envelope() {
    for (method, uri, expected) in [
        (Method::GET, "/api/v1/does-not-exist", StatusCode::NOT_FOUND),
        (
            Method::GET,
            "/api/v1/image/compress",
            StatusCode::METHOD_NOT_ALLOWED,
        ),
        (
            Method::GET,
            "/api/v1/artifacts/%FF",
            StatusCode::BAD_REQUEST,
        ),
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected, "{uri}");
        assert_eq!(
            response.headers()[CONTENT_TYPE],
            "application/json",
            "{uri}"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let document: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(document["success"], false);
        assert!(document["code"].is_string());
        let serialized = document.to_string();
        assert!(!serialized.contains("/tmp/"));
        assert!(!serialized.contains("rtools-api-"));
    }
}

#[tokio::test]
async fn unsupported_conversion_targets_and_ineffective_quality_fail_closed() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for target in ["avif", "ico"] {
        let before = request_directories();
        let (status, document) = json_response(
            &app(),
            multipart(
                "/api/v1/image/convert",
                vec![
                    Part::Text {
                        field: "format",
                        value: target,
                    },
                    Part::File {
                        field: "file",
                        name: "input.png",
                        content_type: "image/png",
                        bytes: png_bytes(2, 2),
                    },
                ],
            ),
        )
        .await;
        assert_structured(status, &document, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(request_directories(), before);
    }

    for target in ["png", "webp", "tiff", "bmp", "gif"] {
        let (status, document) = json_response(
            &app(),
            multipart(
                "/api/v1/image/convert",
                vec![
                    Part::Text {
                        field: "format",
                        value: target,
                    },
                    Part::Text {
                        field: "quality",
                        value: "50",
                    },
                    Part::File {
                        field: "file",
                        name: "input.png",
                        content_type: "image/png",
                        bytes: png_bytes(2, 2),
                    },
                ],
            ),
        )
        .await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
        assert_eq!(document["code"], "INVALID_INPUT");
    }

    let (status, document) = json_response(
        &app(),
        multipart(
            "/api/v1/image/compress",
            vec![
                Part::Text {
                    field: "format",
                    value: "png",
                },
                Part::Text {
                    field: "quality",
                    value: "50",
                },
                Part::File {
                    field: "file",
                    name: "input.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
            ],
        ),
    )
    .await;
    assert_structured(status, &document, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn zero_image_quality_is_invalid_before_request_storage() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for (path, format) in [
        ("/api/v1/image/compress", None),
        ("/api/v1/image/convert", Some("jpeg")),
    ] {
        let mut parts = vec![
            Part::Text {
                field: "quality",
                value: "0",
            },
            Part::File {
                field: "file",
                name: "input.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            },
        ];
        if let Some(format) = format {
            parts.insert(
                0,
                Part::Text {
                    field: "format",
                    value: format,
                },
            );
        }
        let before = request_directories();
        let (status, document) = json_response(&app(), multipart(path, parts)).await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
        assert_eq!(document["code"], "INVALID_INPUT");
        assert_eq!(request_directories(), before);
    }
}

#[tokio::test]
async fn conversion_format_matrix_is_truthful_and_jpeg_quality_is_effective() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let app = app();
    for target in ["jpeg", "png", "webp", "tiff", "bmp", "gif"] {
        let (input_name, input_bytes) = if target == "bmp" {
            ("input.png", png_bytes(16, 16))
        } else {
            let mut bytes = Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(16, 16)
                .write_to(&mut bytes, ImageFormat::Bmp)
                .unwrap();
            ("input.bmp", bytes.into_inner())
        };
        let (status, document) = json_response(
            &app,
            multipart(
                "/api/v1/image/convert",
                vec![
                    Part::Text {
                        field: "format",
                        value: target,
                    },
                    Part::File {
                        field: "file",
                        name: input_name,
                        content_type: "application/octet-stream",
                        bytes: input_bytes,
                    },
                ],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{target}: {document}");
        let artifact = download(&app, &document).await;
        assert!(image::load_from_memory(&artifact).is_ok(), "{target}");
    }

    let mut jpeg_outputs = Vec::new();
    for quality in ["1", "100"] {
        let (status, document) = json_response(
            &app,
            multipart(
                "/api/v1/image/convert",
                vec![
                    Part::Text {
                        field: "format",
                        value: "jpeg",
                    },
                    Part::Text {
                        field: "quality",
                        value: quality,
                    },
                    Part::File {
                        field: "file",
                        name: "input.png",
                        content_type: "image/png",
                        bytes: png_bytes(64, 64),
                    },
                ],
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{quality}: {document}");
        jpeg_outputs.push(download(&app, &document).await);
    }
    assert_ne!(jpeg_outputs[0], jpeg_outputs[1]);
}

#[tokio::test]
async fn malformed_processor_inputs_return_sanitized_client_errors() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    for (path, name, bytes) in [
        (
            "/api/v1/image/compress",
            "bad.png",
            b"not an image".to_vec(),
        ),
        ("/api/v1/pdf/compress", "bad.pdf", b"not a pdf".to_vec()),
    ] {
        let (status, document) = json_response(
            &app(),
            multipart(
                path,
                vec![Part::File {
                    field: "file",
                    name,
                    content_type: "application/octet-stream",
                    bytes,
                }],
            ),
        )
        .await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
        let serialized = document.to_string();
        assert!(!serialized.contains("/tmp/"), "{serialized}");
        assert!(!serialized.contains("rtools-api-request-"), "{serialized}");
        assert!(!serialized.contains(":\\\\"), "{serialized}");
    }
}

#[tokio::test]
async fn configured_upload_limit_rejects_before_request_file_creation() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let before = request_directories();
    let mut config = rtools_core::AppConfig::default();
    config.api.max_upload_size = 128;
    let app = build_router(config).unwrap();
    let response = app
        .oneshot(multipart(
            "/api/v1/image/compress",
            vec![Part::File {
                field: "file",
                name: "large.png",
                content_type: "image/png",
                bytes: vec![0; 256],
            }],
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(response.headers()[CONTENT_TYPE], "application/json");
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let document: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(document["code"], "RESOURCE_LIMIT_EXCEEDED");
    assert_eq!(request_directories(), before);
}

#[tokio::test]
async fn configured_upload_limit_also_applies_to_text_fields() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let before = request_directories();
    let mut config = rtools_core::AppConfig::default();
    config.api.max_upload_size = 192;
    let app = build_router(config).unwrap();
    let oversized = "9".repeat(512);
    let (status, document) = json_response(
        &app,
        multipart(
            "/api/v1/image/resize",
            vec![Part::Text {
                field: "width",
                value: &oversized,
            }],
        ),
    )
    .await;

    assert_structured(status, &document, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(document["code"], "RESOURCE_LIMIT_EXCEEDED", "{document}");
    assert_eq!(request_directories(), before);
}

#[tokio::test]
async fn artifact_store_removes_published_files_when_server_state_drops() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.bin");
    std::fs::write(&source, b"artifact bytes").unwrap();
    let store = ArtifactStore::new().unwrap();
    let root = store.root.path().to_path_buf();
    let artifact = store
        .publish(
            &source,
            "source.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(root.join(artifact.id)).unwrap(),
        b"artifact bytes"
    );

    drop(store);

    assert!(!root.exists());
}

#[tokio::test]
async fn request_and_artifact_storage_are_create_new_and_clean_failed_attempts() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let request = handlers::RequestFiles::new().unwrap();
    let request_root = request.path().to_path_buf();
    let upload = request.write(7, "bin", b"first").unwrap();
    assert!(request.write(7, "bin", b"second").is_err());
    assert_eq!(std::fs::read(upload).unwrap(), b"first");
    drop(request);
    assert!(!request_root.exists());

    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.bin");
    std::fs::write(&source, b"artifact bytes").unwrap();
    let store = ArtifactStore::new().unwrap();
    let first = store
        .publish(
            &source,
            "same.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .await
        .unwrap();
    let second = store
        .publish(
            &source,
            "same.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .await
        .unwrap();
    assert_ne!(first.id, second.id);
    let before_failure = std::fs::read_dir(store.root.path()).unwrap().count();
    assert!(store
        .publish(
            &source_dir.path().join("missing.bin"),
            "missing.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .await
        .is_err());
    assert_eq!(
        std::fs::read_dir(store.root.path()).unwrap().count(),
        before_failure
    );
}

#[test]
fn artifact_identifiers_are_single_opaque_path_components() {
    for candidate in [
        "../artifact",
        "/absolute",
        "C:\\absolute",
        "a/b",
        "a\\b",
        "",
    ] {
        assert!(
            !handlers::artifact::valid_artifact_id(candidate),
            "{candidate}"
        );
    }
    assert!(handlers::artifact::valid_artifact_id(
        "artifact-0123456789abcdef0123456789abcdef"
    ));
    assert!(!handlers::artifact::valid_artifact_id("artifact-Ab3_9.png"));
}

#[tokio::test]
async fn artifact_downloads_use_authoritative_metadata_and_bounded_stream_chunks() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.bin");
    let bytes = vec![0x5a; 256 * 1024 + 17];
    tokio::fs::write(&source, &bytes).await.unwrap();
    let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
    let artifact = state
        .artifacts
        .publish(
            &source,
            "résumé\r\nInjected: yes.heic".to_string(),
            "image/heic".to_string(),
        )
        .await
        .unwrap();
    let unknown = state
        .artifacts
        .publish(
            &source,
            "unknown.extension".to_string(),
            "application/x-rtools-test".to_string(),
        )
        .await
        .unwrap();
    assert!(handlers::artifact::valid_artifact_id(&artifact.id));
    assert_eq!(artifact.id.len(), 41);
    assert_ne!(artifact.id, unknown.id);
    let router = Router::new()
        .route("/api/v1/artifacts/:id", get(handlers::artifact::download))
        .with_state(state);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(&artifact.download_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/heic");
    assert_eq!(
        response.headers()[header::CONTENT_LENGTH],
        bytes.len().to_string()
    );
    let disposition = response.headers()[header::CONTENT_DISPOSITION]
        .to_str()
        .unwrap();
    assert!(disposition.contains("filename*=UTF-8''"));
    assert!(!disposition.contains('\r'));
    assert!(!disposition.contains('\n'));

    let mut stream = response.into_body().into_data_stream();
    let mut received = Vec::new();
    let mut chunks = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        assert!(chunk.len() <= 64 * 1024);
        chunks += 1;
        received.extend_from_slice(&chunk);
    }
    assert!(chunks > 1);
    assert_eq!(received, bytes);

    let head = router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::HEAD)
                .uri(&artifact.download_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
    assert_eq!(head.headers()[header::CONTENT_TYPE], "image/heic");
    assert!(to_bytes(head.into_body(), usize::MAX)
        .await
        .unwrap()
        .is_empty());

    let unknown_response = router
        .oneshot(
            Request::builder()
                .uri(&unknown.download_url)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        unknown_response.headers()[header::CONTENT_TYPE],
        "application/x-rtools-test"
    );
}

#[tokio::test]
async fn artifact_publication_batch_rolls_back_a_late_failure() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.bin");
    tokio::fs::write(&source, b"first").await.unwrap();
    let missing = source_dir.path().join("missing.bin");
    let store = ArtifactStore::new().unwrap();
    let root = store.root.path().to_path_buf();
    let result = store
        .publish_batch(vec![
            handlers::artifact::PendingArtifact {
                source: &source,
                name: "first.bin".to_string(),
                media_type: "application/x-test".to_string(),
            },
            handlers::artifact::PendingArtifact {
                source: &missing,
                name: "missing.bin".to_string(),
                media_type: "application/octet-stream".to_string(),
            },
        ])
        .await;
    assert!(result.is_err());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 0);
    assert!(store.records.read().await.is_empty());
}

#[tokio::test]
async fn artifact_publication_cancellation_removes_every_create_new_path() {
    use handlers::artifact::ArtifactPublishStage;

    for (stage, expected_files) in [
        (ArtifactPublishStage::Copy, 1),
        (ArtifactPublishStage::Sync, 1),
        (ArtifactPublishStage::BetweenCopies, 1),
        (ArtifactPublishStage::RecordLock, 2),
    ] {
        let source_dir = tempfile::tempdir().unwrap();
        let first = source_dir.path().join("first.bin");
        let second = source_dir.path().join("second.bin");
        tokio::fs::write(&first, vec![0x11; 64 * 1024])
            .await
            .unwrap();
        tokio::fs::write(&second, vec![0x22; 64 * 1024])
            .await
            .unwrap();
        let store = Arc::new(ArtifactStore::new().unwrap());
        let pause = store.pause_publication_at(stage, 1);
        let task_store = Arc::clone(&store);
        let task = tokio::spawn(async move {
            task_store
                .publish_batch(vec![
                    handlers::artifact::PendingArtifact {
                        source: &first,
                        name: "first.bin".to_string(),
                        media_type: "application/octet-stream".to_string(),
                    },
                    handlers::artifact::PendingArtifact {
                        source: &second,
                        name: "second.bin".to_string(),
                        media_type: "application/octet-stream".to_string(),
                    },
                ])
                .await
        });

        pause.wait_until_entered().await;
        assert_eq!(
            std::fs::read_dir(store.root.path()).unwrap().count(),
            expected_files,
            "wrong number of registered paths at {stage:?}"
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(
            std::fs::read_dir(store.root.path()).unwrap().count(),
            0,
            "cancellation leaked an artifact at {stage:?}"
        );
        assert!(store.records.read().await.is_empty());
    }
}

#[tokio::test]
async fn artifact_publication_reports_cleanup_failure_as_rollback_failed() {
    let source_dir = tempfile::tempdir().unwrap();
    let first = source_dir.path().join("first.bin");
    let second = source_dir.path().join("second.bin");
    tokio::fs::write(&first, b"first").await.unwrap();
    tokio::fs::write(&second, b"second").await.unwrap();
    let store = ArtifactStore::new().unwrap();
    store.fail_publication_copy_on(2);
    store.fail_publication_delete_on(1);

    let error = store
        .publish_batch(vec![
            handlers::artifact::PendingArtifact {
                source: &first,
                name: "first.bin".to_string(),
                media_type: "application/octet-stream".to_string(),
            },
            handlers::artifact::PendingArtifact {
                source: &second,
                name: "second.bin".to_string(),
                media_type: "application/octet-stream".to_string(),
            },
        ])
        .await
        .unwrap_err();

    assert_eq!(error.code(), rtools_core::ErrorCode::RollbackFailed);
    assert!(store.records.read().await.is_empty());
    assert_eq!(std::fs::read_dir(store.root.path()).unwrap().count(), 0);
}

#[tokio::test]
async fn graceful_server_shutdown_drops_the_artifact_store() {
    let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
    let root = state.artifacts.root.path().to_path_buf();
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.bin");
    tokio::fs::write(&source, b"artifact").await.unwrap();
    state
        .artifacts
        .publish(
            &source,
            "source.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .await
        .unwrap();
    let router = build_router_with_state(state.clone(), 1024 * 1024);
    drop(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(serve_with_shutdown(listener, router, async move {
        let _ = shutdown_rx.await;
    }));
    shutdown_tx.send(()).unwrap();
    server.await.unwrap().unwrap();
    assert!(!root.exists());
}

#[test]
fn non_default_unenforceable_image_config_fails_closed_at_startup() {
    let mut configs = Vec::new();
    let mut config = rtools_core::AppConfig::default();
    config.image.webp_lossless = false;
    configs.push(config);
    let mut config = rtools_core::AppConfig::default();
    config.image.avif_enabled = false;
    configs.push(config);
    let mut config = rtools_core::AppConfig::default();
    config.image.max_dimension += 1;
    configs.push(config);
    let mut config = rtools_core::AppConfig::default();
    config.image.jpeg_quality -= 1;
    configs.push(config);
    let mut config = rtools_core::AppConfig::default();
    config.image.png_compression -= 1;
    configs.push(config);
    let mut config = rtools_core::AppConfig::default();
    config.image.dither = true;
    configs.push(config);
    for config in configs {
        assert!(AppState::new(config).is_err());
    }
}

#[tokio::test]
async fn rename_preflights_reserved_names_and_batch_collisions_without_artifacts() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    assert!(rtools_ai::rename::validate_unique_portable_filenames(&[
        "\u{1c90}.png".to_string(),
        "\u{10d0}.png".to_string(),
    ])
    .is_err());
    for (name, pattern) in [("COM1.png", concat!("{", "name}")), ("first.png", "same")] {
        let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
        let router = build_router_with_state(state.clone(), 1024 * 1024);
        let mut parts = vec![Part::File {
            field: "files",
            name,
            content_type: "image/png",
            bytes: png_bytes(2, 2),
        }];
        if pattern == "same" {
            parts.push(Part::File {
                field: "files",
                name: "second.png",
                content_type: "image/png",
                bytes: png_bytes(2, 2),
            });
        }
        parts.push(Part::Text {
            field: "pattern",
            value: pattern,
        });
        let (status, document) =
            json_response(&router, multipart("/api/v1/ai/rename", parts)).await;
        assert_structured(status, &document, StatusCode::BAD_REQUEST);
        assert!(state.artifacts.records.read().await.is_empty());
        assert_eq!(
            std::fs::read_dir(state.artifacts.root.path())
                .unwrap()
                .count(),
            0
        );
    }
}

#[tokio::test]
async fn rename_uses_one_isolated_batch_when_client_names_resemble_staging_names() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let before = request_directories();
    let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
    let router = build_router_with_state(state.clone(), 1024 * 1024);
    let (status, document) = json_response(
        &router,
        multipart(
            "/api/v1/ai/rename",
            vec![
                Part::File {
                    field: "files",
                    name: "first.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
                Part::File {
                    field: "files",
                    name: "upload-00000002.png",
                    content_type: "image/png",
                    bytes: png_bytes(3, 2),
                },
                Part::File {
                    field: "files",
                    name: "third.png",
                    content_type: "image/png",
                    bytes: png_bytes(4, 2),
                },
                Part::Text {
                    field: "pattern",
                    value: "{name}",
                },
            ],
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "{document}");
    assert_eq!(
        document["results"]["names"],
        serde_json::json!(["first.png", "upload-00000002.png", "third.png"])
    );
    assert_eq!(
        document["results"]["artifacts"].as_array().unwrap().len(),
        3
    );
    for (artifact, width) in document["results"]["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .zip([2, 3, 4])
    {
        let id = artifact["id"].as_str().unwrap();
        assert_eq!(
            image::load_from_memory(&std::fs::read(state.artifacts.root.path().join(id)).unwrap())
                .unwrap()
                .width(),
            width
        );
    }
    assert_eq!(state.artifacts.records.read().await.len(), 3);
    assert_eq!(
        std::fs::read_dir(state.artifacts.root.path())
            .unwrap()
            .count(),
        3
    );
    assert_eq!(request_directories(), before);
    assert!(!document
        .to_string()
        .contains(state.artifacts.root.path().to_string_lossy().as_ref()));
}

#[tokio::test]
async fn rest_rename_uses_shared_superscript_and_component_length_validation() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let before = request_directories();
    for (name, pattern) in [
        ("COM¹.png", ["{", "name", "}"].concat()),
        ("input.png", "a".repeat(256)),
    ] {
        let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
        let router = build_router_with_state(state.clone(), 1024 * 1024);
        let (status, document) = json_response(
            &router,
            multipart(
                "/api/v1/ai/rename",
                vec![
                    Part::File {
                        field: "files",
                        name,
                        content_type: "image/png",
                        bytes: png_bytes(2, 2),
                    },
                    Part::Text {
                        field: "pattern",
                        value: &pattern,
                    },
                ],
            ),
        )
        .await;

        assert_structured(status, &document, StatusCode::BAD_REQUEST);
        assert_eq!(document["code"], "INVALID_INPUT");
        assert!(state.artifacts.records.read().await.is_empty());
        assert_eq!(
            std::fs::read_dir(state.artifacts.root.path())
                .unwrap()
                .count(),
            0
        );
    }
    assert_eq!(request_directories(), before);
}

#[tokio::test]
async fn rest_rename_cleans_a_late_publication_failure_without_artifacts() {
    let _guard = REQUEST_DIRECTORY_TEST_LOCK.lock().await;
    let before = request_directories();
    let state = Arc::new(AppState::new(rtools_core::AppConfig::default()).unwrap());
    state.artifacts.fail_publication_copy_on(2);
    let router = build_router_with_state(state.clone(), 1024 * 1024);
    let (status, document) = json_response(
        &router,
        multipart(
            "/api/v1/ai/rename",
            vec![
                Part::File {
                    field: "files",
                    name: "first.png",
                    content_type: "image/png",
                    bytes: png_bytes(2, 2),
                },
                Part::File {
                    field: "files",
                    name: "second.png",
                    content_type: "image/png",
                    bytes: png_bytes(3, 2),
                },
                Part::File {
                    field: "files",
                    name: "third.png",
                    content_type: "image/png",
                    bytes: png_bytes(4, 2),
                },
                Part::Text {
                    field: "pattern",
                    value: "renamed_{index}",
                },
            ],
        ),
    )
    .await;

    assert_structured(status, &document, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(document["code"], "PROCESSING_FAILED");
    assert!(state.artifacts.records.read().await.is_empty());
    assert_eq!(
        std::fs::read_dir(state.artifacts.root.path())
            .unwrap()
            .count(),
        0
    );
    assert_eq!(request_directories(), before);
}
