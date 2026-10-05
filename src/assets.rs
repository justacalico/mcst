//! Embedded Flutter web build, served with SPA fallback.

use axum::extract::Path as AxumPath;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use include_dir::{include_dir, Dir};

/// The compiled Flutter web app, embedded at compile time.
static DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");

/// Serve a file from the embedded bundle; falls back to index.html for
/// client-side routes.
pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    serve_path(if path.is_empty() { "index.html" } else { path })
}

pub async fn serve_file(AxumPath(path): AxumPath<String>) -> Response {
    serve_path(&path)
}

fn serve_path(path: &str) -> Response {
    match DIST.get_file(path) {
        Some(f) => file_response(f),
        None => match DIST.get_file("index.html") {
            Some(f) => file_response(f),
            None => (
                StatusCode::SERVICE_UNAVAILABLE,
                "mcst frontend not embedded — build flutter web first",
            )
                .into_response(),
        },
    }
}

fn file_response(f: &include_dir::File) -> Response {
    let mime = mime_guess::from_path(f.path()).first_or_octet_stream();
    let mut res = (
        [(header::CONTENT_TYPE, mime.as_ref())],
        f.contents().to_vec(),
    )
        .into_response();
    // Vite/Flutter-hashed assets get immutable caching; html does not.
    let name = f.path().to_string_lossy();
    let cache = if name.ends_with(".html") || name == "index.html" {
        HeaderValue::from_static("no-cache")
    } else {
        HeaderValue::from_static("public, max-age=31536000, immutable")
    };
    res.headers_mut().insert(header::CACHE_CONTROL, cache);
    res
}
