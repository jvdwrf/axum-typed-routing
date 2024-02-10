#![allow(unused)]
use aide::axum::ApiRouter;
use axum::extract::{Json, State};
use axum_typed_routing::TypedApiRouter;
use axum_typed_routing_macros::api_route;

#[api_route(GET "/item/:id?amount&offset" {
    summary: "Get an item",
    description: "Get an item by id",
    id: "get-item",
    tags: ["items"],
    hidden: false
})]
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
    let router: ApiRouter = ApiRouter::new()
        .typed_api_route(item_handler)
        .with_state("state".to_string());
}
