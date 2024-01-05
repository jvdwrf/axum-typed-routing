#![allow(unused)]
#![allow(clippy::extra_unused_type_parameters)]

use axum::{extract::State, Form, Json};
use axum_typed_routing::TypedRouter;
use axum_typed_routing_macros::route;

/// This is a handler that is documented!
#[route(GET "/hello/:id?user_id&name")]
async fn my_handler<T: 'static>(
    mut id: u32,
    user_id: String,
    name: String,
    State(state): State<String>,
    hello: State<String>,
    Json(mut json): Json<u32>,
) -> String {
    format!("Hello, {}!", name)
}

#[route(POST "/hello")]
async fn my_handler2(state: State<String>) -> String {
    String::from("Hello!")
}

#[route(POST "/hello")]
async fn my_handler3(mut form: Form<u32>) -> String {
    String::from("Hello!")
}

#[test]
fn test_normal() {
    let _: axum::Router = axum::Router::new()
        .typed_route(my_handler::<u32>)
        .typed_route(my_handler2)
        .with_state("state".to_string());

    let (path, method_router) = my_handler::<u32>();
    assert_eq!(path, "/hello/:id");

    let (path, method_router) = my_handler2();
    assert_eq!(path, "/hello");
}

#[cfg(feature = "aide")]
mod aide_support {
    use super::*;
    use aide::{axum::ApiRouter, openapi::OpenApi};
    use axum_typed_routing::TypedApiRouter;
    use axum_typed_routing_macros::api_route;

    /// get-summary
    ///
    /// get-description
    #[api_route(GET "/hello")]
    async fn get_hello(state: State<String>) -> String {
        String::from("Hello!")
    }

    /// post-summary
    ///
    /// post-description
    #[api_route(POST "/hello")]
    async fn post_hello(state: State<String>) -> String {
        String::from("Hello!")
    }

    #[test]
    fn test_aide() {
        let router: aide::axum::ApiRouter = aide::axum::ApiRouter::new()
            .typed_route(my_handler2)
            .typed_api_route(get_hello)
            .with_state("state".to_string());

        let (path, method_router) = get_hello();
        assert_eq!(path, "/hello");

        let (path, method_router) = post_hello();
        assert_eq!(path, "/hello");
    }

    #[test]
    fn summary_and_description_are_generated_from_doc_comments() {
        let router = ApiRouter::new()
            .typed_api_route(get_hello)
            .typed_api_route(post_hello);
        let mut api = OpenApi::default();
        router.finish_api(&mut api);

        let get_op = path_item(&api, "/hello").get.as_ref().unwrap();
        let post_op = path_item(&api, "/hello").post.as_ref().unwrap();

        assert_eq!(get_op.summary, Some(" get-summary".to_string()));
        assert_eq!(get_op.description, Some(" get-description".to_string()));
        assert!(get_op.tags.is_empty());

        assert_eq!(post_op.summary, Some(" post-summary".to_string()));
        assert_eq!(post_op.description, Some(" post-description".to_string()));
        assert!(post_op.tags.is_empty());
    }

    fn path_item<'a>(api: &'a OpenApi, path: &str) -> &'a aide::openapi::PathItem {
        api.paths
            .as_ref()
            .unwrap()
            .iter()
            .find(|(p, _)| *p == path)
            .unwrap()
            .1
            .as_item()
            .unwrap()
    }

    /// unused-summary
    /// 
    /// unused-description
    #[api_route(GET "/hello" {
        summary: "MySummary",
        description: "MyDescription",
        hidden: false,
        tags: ["MyTag1", "MyTag2"],
    })]
    async fn get_gello_with_attributes(state: State<String>) -> String {
        String::from("Hello!")
    }

    #[test]
    fn summary_and_description_are_generated_from_attributes() {
        let router = ApiRouter::new().typed_api_route(get_gello_with_attributes);
        let mut api = OpenApi::default();
        router.finish_api(&mut api);

        let get_op = path_item(&api, "/hello").get.as_ref().unwrap();

        assert_eq!(get_op.summary, Some("MySummary".to_string()));
        assert_eq!(get_op.description, Some("MyDescription".to_string()));
        assert_eq!(get_op.tags, vec!["MyTag1".to_string(), "MyTag2".to_string()]);
    }
}
