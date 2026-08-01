#![allow(unused)]
use axum::extract::{Json, State};
use axum_typed_routing::{route, uri, TypedRouter};

#[route(GET "/item/{id}?amount&offset")]
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

    // Type-safely construct a uri using the handy uri!() macro:
    let uri = uri!(item_handler(id = 1, amount = Some(2), offset = _));
    assert_eq!(uri, "/item/1?amount=2");
}
