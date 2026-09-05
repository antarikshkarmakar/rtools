use super::*;
use axum::{
    body::{to_bytes, Body},
    http::{header::CONTENT_TYPE, Request, StatusCode},
};
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

#[test]
fn artifact_store_removes_published_files_when_server_state_drops() {
    let source_dir = tempfile::tempdir().unwrap();
    let source = source_dir.path().join("source.bin");
    std::fs::write(&source, b"artifact bytes").unwrap();
    let store = ArtifactStore {
        root: tempfile::Builder::new()
            .prefix("rtools-api-artifact-lifecycle-")
            .tempdir()
            .unwrap(),
    };
    let root = store.root.path().to_path_buf();
    let artifact = store
        .publish(
            &source,
            "source.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .unwrap();
    assert_eq!(
        std::fs::read(root.join(artifact.id)).unwrap(),
        b"artifact bytes"
    );

    drop(store);

    assert!(!root.exists());
}

#[test]
fn request_and_artifact_storage_are_create_new_and_clean_failed_attempts() {
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
    let store = ArtifactStore {
        root: tempfile::Builder::new()
            .prefix("rtools-api-artifact-failure-")
            .tempdir()
            .unwrap(),
    };
    let first = store
        .publish(
            &source,
            "same.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .unwrap();
    let second = store
        .publish(
            &source,
            "same.bin".to_string(),
            "application/octet-stream".to_string(),
        )
        .unwrap();
    assert_ne!(first.id, second.id);
    let before_failure = std::fs::read_dir(store.root.path()).unwrap().count();
    assert!(store
        .publish(
            &source_dir.path().join("missing.bin"),
            "missing.bin".to_string(),
            "application/octet-stream".to_string(),
        )
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
    assert!(handlers::artifact::valid_artifact_id("artifact-Ab3_9.png"));
}
