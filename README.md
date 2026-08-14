# openapi-route

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

Handler-local OpenAPI metadata for Rust HTTP services.

The project is split into three crates:

- openapi-route contains framework-neutral route metadata and document generation.
- openapi-route-macros generates explicit route constants from handler annotations.
- openapi-route-axum mounts the generated document and Swagger UI in Axum.

Route metadata is assembled through explicit service-owned catalogs. The library does
not use a process-global registry or constructor-based registration.

## License

Copyright © 2026 DataRoad Inc, Delaware, USA, trading as Legra.

Licensed under either the [MIT license](LICENSE-MIT) or the
[Apache License, Version 2.0](LICENSE-APACHE), at your option.
