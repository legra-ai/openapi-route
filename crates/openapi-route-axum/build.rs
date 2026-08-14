//! Fetch the pinned Swagger UI distribution at build time.
//!
//! The UI assets are served from the built binary — never from a
//! third-party CDN — so pages documented on customer-bound hostnames
//! load no external script, leak no visitor traffic, and keep working
//! on air-gapped deployments. The assets are not vendored into git;
//! this build script downloads the pinned npm tarball, verifies its
//! SHA-256, and extracts exactly the files the shell references into
//! `OUT_DIR`, where `lib.rs` embeds them with `include_bytes!`.
//!
//! Offline builds: set `OPENAPI_ROUTE_SWAGGER_UI_TARBALL` to a local
//! copy of the tarball (same pinned version; the checksum is still
//! enforced).

use std::io::Read;
use std::path::{Path, PathBuf};

/// Pinned Swagger UI version. Upgrading is a deliberate change to
/// both constants, reviewed like any dependency bump.
const SWAGGER_UI_VERSION: &str = "5.32.12";

/// SHA-256 of the npm tarball for [`SWAGGER_UI_VERSION`].
const TARBALL_SHA256: &str = "cb6eaceaa90f428bc1ee2cdf55d48dfa60ad59a1c65338edf359f9e5d6a2dd60";

/// The files the HTML shell references, extracted from the tarball's
/// `package/` root.
const ASSET_FILES: &[&str] = &[
    "swagger-ui.css",
    "swagger-ui-bundle.js",
    "swagger-ui-standalone-preset.js",
    "swagger-ui-bundle.js.LICENSE.txt",
    "favicon-32x32.png",
    "favicon-16x16.png",
];

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=OPENAPI_ROUTE_SWAGGER_UI_TARBALL");
    println!("cargo::rustc-env=OPENAPI_ROUTE_SWAGGER_UI_VERSION={SWAGGER_UI_VERSION}");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let asset_dir = out_dir.join("swagger-ui");

    if assets_present(&asset_dir) {
        return;
    }

    let tarball = obtain_tarball();
    verify_sha256(&tarball);
    extract_assets(&tarball, &asset_dir);
}

/// All expected files already extracted (a previous build of the same
/// pinned version — `OUT_DIR` is per-version by way of the checksum
/// gate below, so stale assets cannot survive an upgrade unnoticed).
fn assets_present(asset_dir: &Path) -> bool {
    let marker = asset_dir.join(format!(".version-{SWAGGER_UI_VERSION}"));
    marker.is_file()
        && ASSET_FILES
            .iter()
            .all(|name| asset_dir.join(name).is_file())
}

/// The tarball bytes: a local override for offline builds, else the
/// npm registry.
fn obtain_tarball() -> Vec<u8> {
    if let Ok(local) = std::env::var("OPENAPI_ROUTE_SWAGGER_UI_TARBALL") {
        return std::fs::read(&local).unwrap_or_else(|e| {
            panic!("OPENAPI_ROUTE_SWAGGER_UI_TARBALL={local} is not readable: {e}")
        });
    }

    let url = format!(
        "https://registry.npmjs.org/swagger-ui-dist/-/swagger-ui-dist-{SWAGGER_UI_VERSION}.tgz"
    );
    let response = ureq::get(&url).call().unwrap_or_else(|e| {
        panic!(
            "downloading {url} failed: {e}\n\
             offline? set OPENAPI_ROUTE_SWAGGER_UI_TARBALL to a local copy of the tarball"
        )
    });
    let mut bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut bytes)
        .unwrap_or_else(|e| panic!("reading {url} failed: {e}"));
    bytes
}

/// Refuse any tarball that is not byte-identical to the pinned one.
fn verify_sha256(tarball: &[u8]) {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(tarball);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        hex, TARBALL_SHA256,
        "swagger-ui-dist tarball checksum mismatch: expected {TARBALL_SHA256}, got {hex}; \
         refusing to embed unverified assets"
    );
}

/// Extract exactly [`ASSET_FILES`] from `package/` into `asset_dir`.
fn extract_assets(tarball: &[u8], asset_dir: &Path) {
    std::fs::create_dir_all(asset_dir)
        .unwrap_or_else(|e| panic!("creating {}: {e}", asset_dir.display()));

    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(tarball));
    let mut remaining: Vec<&str> = ASSET_FILES.to_vec();
    for entry in archive.entries().expect("tarball entries") {
        let mut entry = entry.expect("tarball entry");
        let path = entry.path().expect("entry path").into_owned();
        let Ok(name) = path.strip_prefix("package") else {
            continue;
        };
        let Some(name) = name.to_str() else { continue };
        let Some(index) = remaining.iter().position(|want| *want == name) else {
            continue;
        };
        let target = asset_dir.join(name);
        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .unwrap_or_else(|e| panic!("reading {name} from tarball: {e}"));
        std::fs::write(&target, contents)
            .unwrap_or_else(|e| panic!("writing {}: {e}", target.display()));
        remaining.swap_remove(index);
    }
    assert!(
        remaining.is_empty(),
        "swagger-ui-dist {SWAGGER_UI_VERSION} tarball is missing expected files: {remaining:?}"
    );

    let marker = asset_dir.join(format!(".version-{SWAGGER_UI_VERSION}"));
    std::fs::write(&marker, TARBALL_SHA256)
        .unwrap_or_else(|e| panic!("writing {}: {e}", marker.display()));
}
