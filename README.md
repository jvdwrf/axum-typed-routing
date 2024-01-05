A library for creating statically-typed handlers in axum using macros, similar to Rocket. 
See the docs for the [`route`] macro and the [`TypedRouter`] trait for more information. Currently, this library only supports axum `0.6`, but can easily be updated to `0.7`.

[![Crates.io](https://img.shields.io/crates/v/axum-typed-routing)](https://crates.io/crates/axum-typed-routing)
[![Documentation](https://docs.rs/axum-typed-routing/badge.svg)](https://docs.rs/axum-typed-routing)

# Example
```rust
use axum::extract::{State, Json};
use axum_typed_routing::{TypedRouter, route};

#[route(GET "/item/:id?amount&offset")]
async fn item_handler(
   id: u32,
   amount: Option<u32>,
   offset: Option<u32>,
   State(state): State<String>,
   Json(json): Json<u32>,
) -> String {
   todo!("handle request")
}

fn main() {
    let router: axum::Router = axum::Router::new()
        .typed_route(item_handler)
        .with_state("state".to_string());
}
```

# Aide support
This library also supports [aide](https://docs.rs/aide/0.12.0/aide/index.html)! To use it, enable the `aide` feature.
This adds the macro [`api_route`] and the trait [`TypedApiRouter`].