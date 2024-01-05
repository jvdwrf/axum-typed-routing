#![allow(unused)]
#![allow(clippy::extra_unused_type_parameters)]


use axum::{extract::State, Json};
use axum_typed_routing::TypedRouter;
use axum_typed_routing_macros::{api_route, route};

/// This is a handler that is documented!
#[route(GET "/hello/:id?user_id&name")]
async fn my_handler<T: 'static>(
    id: u32,
    user_id: String,
    name: String,
    State(state): State<String>,
    Json(json): Json<u32>,
) -> String {
    format!("Hello, {}!", name)
}

#[route(POST "/hello")]
async fn my_handler2(state: State<String>) -> String {
    String::from("Hello!")
}

#[cfg(feature = "aide")]
#[api_route(GET "/hello")]
async fn my_handler3(state: State<String>) -> String {
    String::from("Hello!")
}

#[test]
fn test_normal() {
    let _: axum::Router = axum::Router::new()
        .typed_route(my_handler::<u32>)
        .typed_route(my_handler2)
        .with_state("state".to_string());
}


#[cfg(feature = "aide")]
#[test]
fn test_aide() {
    use axum_typed_routing::TypedApiRouter;

    let _: aide::axum::ApiRouter = aide::axum::ApiRouter::new()
        .typed_route(my_handler::<u32>)
        .typed_route(my_handler2)
        .typed_api_route(my_handler3)
        .with_state("state".to_string());
}