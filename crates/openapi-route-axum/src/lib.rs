//! Axum and Swagger UI integration for the openapi-route metadata crate.
//!
//! The Swagger UI assets are embedded in the binary at build time
//! (see `build.rs`) and every reference in the served HTML is
//! relative, so the same UI works at any mount point — a service
//! root's `/swagger-ui/` or a nested namespace like
//! `/workspace/{ref}/swagger-ui/` — without loading a single byte
//! from a third-party origin.

use axum::Router;
use axum::extract::OriginalUri;
use axum::http::header;
use axum::response::{Html, IntoResponse, Json, Redirect, Response};
use axum::routing::get;
use openapi_route::ApiCatalog;

/// The embedded Swagger UI version (pinned in `build.rs`).
pub const SWAGGER_UI_VERSION: &str = env!("OPENAPI_ROUTE_SWAGGER_UI_VERSION");

macro_rules! asset {
    ($name:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/swagger-ui/", $name)).as_slice()
    };
}

/// Mount Swagger UI and the generated OpenAPI document.
pub fn router<S>(catalog: &'static ApiCatalog) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::<S>::new()
        .route(
            "/openapi.json",
            get(move || async move { Json(catalog.document()) }),
        )
        .merge(swagger_ui_router(catalog.ui_title))
}

/// Mount only the documentation UI: `/swagger-ui/` plus its embedded
/// assets, expecting a sibling `/openapi.json` at the same mount
/// level (every reference the page makes is relative).
pub fn swagger_ui_router<S>(title: &'static str) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::<S>::new()
        .route(
            "/swagger-ui",
            // Relative asset references need the trailing slash; the
            // original URI keeps the redirect correct under nesting.
            get(|OriginalUri(uri): OriginalUri| async move {
                Redirect::permanent(&format!("{}/", uri.path()))
            }),
        )
        .route(
            "/swagger-ui/",
            get(move || async move { Html(swagger_html(title)) }),
        )
        .route(
            "/swagger-ui/swagger-ui.css",
            get(|| async { asset_response("text/css", asset!("swagger-ui.css")) }),
        )
        .route(
            "/swagger-ui/swagger-ui-bundle.js",
            get(|| async { asset_response("text/javascript", asset!("swagger-ui-bundle.js")) }),
        )
        .route(
            "/swagger-ui/swagger-ui-standalone-preset.js",
            get(|| async {
                asset_response("text/javascript", asset!("swagger-ui-standalone-preset.js"))
            }),
        )
        .route(
            "/swagger-ui/swagger-ui-bundle.js.LICENSE.txt",
            get(|| async {
                asset_response("text/plain", asset!("swagger-ui-bundle.js.LICENSE.txt"))
            }),
        )
        .route(
            "/swagger-ui/favicon-32x32.png",
            get(|| async { asset_response("image/png", asset!("favicon-32x32.png")) }),
        )
        .route(
            "/swagger-ui/favicon-16x16.png",
            get(|| async { asset_response("image/png", asset!("favicon-16x16.png")) }),
        )
}

/// An embedded asset with a long-lived cache: the HTML references
/// every asset with a `?v={version}` query, so a version bump busts
/// caches while the unversioned path stays immutable in practice.
fn asset_response(content_type: &'static str, body: &'static [u8]) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        body,
    )
        .into_response()
}

fn swagger_html(title: &str) -> String {
    r#"<!DOCTYPE html>
<html>
<head>
    <title>__OPENAPI_ROUTE_UI_TITLE__</title>
    <link rel="icon" type="image/png" sizes="32x32" href="favicon-32x32.png?v=__V__" />
    <link rel="icon" type="image/png" sizes="16x16" href="favicon-16x16.png?v=__V__" />
    <link rel="stylesheet" type="text/css" href="swagger-ui.css?v=__V__" />
    <style>
        html { box-sizing: border-box; overflow-y: scroll; }
        *, *:before, *:after { box-sizing: inherit; }
        body { margin: 0; background: #fafafa; }
    </style>
</head>
<body>
    <div id="swagger-ui"></div>
    <script src="swagger-ui-bundle.js?v=__V__"></script>
    <script src="swagger-ui-standalone-preset.js?v=__V__"></script>
    <script>
        window.onload = function() {
            window.ui = SwaggerUIBundle({
                url: '../openapi.json',
                dom_id: '#swagger-ui',
                deepLinking: true,
                presets: [
                    SwaggerUIBundle.presets.apis,
                    SwaggerUIStandalonePreset
                ],
                plugins: [SwaggerUIBundle.plugins.DownloadUrl],
                layout: 'StandaloneLayout'
            });
        };
    </script>
</body>
</html>"#
        .replace("__OPENAPI_ROUTE_UI_TITLE__", title)
        .replace("__V__", SWAGGER_UI_VERSION)
}

#[cfg(test)]
mod tests {
    use super::{SWAGGER_UI_VERSION, swagger_html};

    #[test]
    fn swagger_html_uses_catalog_ui_title() {
        let html = swagger_html("WWKG Gateway APIs");
        assert!(html.contains("<title>WWKG Gateway APIs</title>"));
    }

    /// The page must be self-contained: every script, stylesheet, and
    /// icon reference is relative, so nothing loads from a third-party
    /// origin and the same page works at any mount depth.
    #[test]
    fn swagger_html_references_no_external_origin() {
        let html = swagger_html("t");
        assert!(!html.contains("http://"), "external reference: {html}");
        assert!(!html.contains("https://"), "external reference: {html}");
        assert!(!html.contains("src=\"/"), "root-absolute script: {html}");
        assert!(!html.contains("href=\"/"), "root-absolute link: {html}");
    }

    /// The document URL is relative to the mount level, so a nested
    /// `/workspace/{ref}/swagger-ui/` reads its sibling document.
    #[test]
    fn swagger_html_reads_the_sibling_document() {
        assert!(swagger_html("t").contains("url: '../openapi.json'"));
    }

    /// Asset references carry the pinned version for cache busting.
    #[test]
    fn asset_references_carry_the_pinned_version() {
        let html = swagger_html("t");
        assert!(html.contains(&format!("swagger-ui.css?v={SWAGGER_UI_VERSION}")));
        assert!(html.contains(&format!("swagger-ui-bundle.js?v={SWAGGER_UI_VERSION}")));
    }

    /// The embedded assets are the real files, not placeholders.
    #[test]
    fn embedded_assets_are_present() {
        assert!(!asset!("swagger-ui.css").is_empty());
        assert!(asset!("swagger-ui-bundle.js").len() > 100_000);
        assert!(!asset!("swagger-ui-standalone-preset.js").is_empty());
        assert!(!asset!("favicon-32x32.png").is_empty());
    }
}
