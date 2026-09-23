[![Crates.io](https://img.shields.io/crates/v/axum-typed-routing)](https://crates.io/crates/axum-typed-routing)
[![Documentation](https://docs.rs/axum-typed-routing/badge.svg)](https://docs.rs/axum-typed-routing)

# axum-typed-routing

Statically-typed routing macros for [axum](https://github.com/tokio-rs/axum), in the style of Rocket, with optional OpenAPI generation through [aide](https://docs.rs/aide).

Write the route's path, including path and query parameters, next to the handler. The macro checks at compile time that every parameter matches a handler argument, and extracts the parameters into those arguments.

## Installation

```toml
[dependencies]
axum-typed-routing = "0.4"
axum = "0.8"
serde = { version = "1", features = ["derive"] }
```

The generated code uses `serde` directly, so it must be a dependency of your crate.

To generate OpenAPI docs, enable the `aide` feature and add `aide` and `schemars`:

```toml
axum-typed-routing = { version = "0.4", features = ["aide"] }
aide = { version = "0.15", features = ["axum", "axum-query"] }
schemars = "0.9"
```

## Example

```rust
use axum::extract::{Json, State};
use axum_typed_routing::{TypedRouter, route};

#[route(GET "/item/{id}?amount&offset")]
async fn item_handler(
    id: u32,              // path parameter
    amount: Option<u32>,  // optional query parameter
    offset: Option<u32>,  // optional query parameter
    State(state): State<String>,
    Json(json): Json<u32>,
) -> String {
    todo!("handle request")
}

let router: axum::Router = axum::Router::new()
    .typed_route(item_handler)
    .with_state("state".to_string());
```

With the `aide` feature, use `#[api_route(...)]` and `.typed_api_route(...)` on an `aide::axum::ApiRouter`. The handler's doc comments become the operation's summary and description, and you can set tags, security, responses and more in the attribute.

Other features include wildcard paths (`/files/*path`), explicit state types (`with AppState`), generic handlers, and a `debug` mode that wraps the handler in `axum::debug_handler`. See the [documentation](https://docs.rs/axum-typed-routing) for the full guide.

## Pairs well with axum-error-sets

[axum-error-sets](https://github.com/jvdwrf/axum-error-sets) lets each handler declare exactly which HTTP status codes it can return, as a tuple type checked at compile time. Combined with `api_route`, every status code in a handler's error set appears in the OpenAPI documentation with no extra annotations:

```rust
#[api_route(GET "/item/{id}")]
async fn get_item(id: u32) -> ApiResult<Json<Item>, (Unauthorized, NotFound<String>)> {
    // ...
}
```

## License

Licensed under either of MIT or Apache-2.0, at your option.
