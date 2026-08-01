#![allow(unused)]
use aide::{OperationInput, axum::ApiRouter};
use axum::extract::{Json, State};
use axum_typed_routing::TypedApiRouter;
use axum_typed_routing_macros::api_route;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[api_route(GET "/item/{id}?amount&offset" {
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
    json: String,
) -> String {
    todo!("handle request")
}

fn main() {
    let router: ApiRouter = ApiRouter::new()
        .typed_api_route(item_handler)
        .with_state("state".to_string());
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct Integer(pub u32);

impl OperationInput for Integer {}
