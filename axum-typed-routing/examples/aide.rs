#![allow(unused)]
use aide::{axum::ApiRouter, openapi::OpenApi};
use axum::extract::State;
use axum_typed_routing::{TypedApiRouter, api_route};

/// Get an item
///
/// Returns the item with the given id.
#[api_route(GET "/item/{id}?amount&offset" {
    tags: ["items"],
})]
async fn item_handler(
    /// The id of the item to get
    id: u32,
    /// The amount of items to get
    amount: Option<u32>,
    /// The offset of the items to get
    offset: Option<u32>,
    State(state): State<String>,
) -> String {
    todo!("handle request")
}

fn main() {
    let mut api = OpenApi::default();

    let router: axum::Router = ApiRouter::new()
        .typed_api_route(item_handler)
        .with_state("state".to_string())
        .finish_api(&mut api);

    // `api` now describes `GET /item/{id}`, including its parameters.
    println!("{}", serde_json::to_string_pretty(&api).unwrap());
}
