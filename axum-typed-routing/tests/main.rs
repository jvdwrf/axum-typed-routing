#![allow(unused)]
#![allow(clippy::extra_unused_type_parameters)]

use std::net::TcpListener;

use axum::{
    extract::{Path, State},
    routing::get,
    Form, Json,
};
use axum_test::TestServer;
use axum_typed_routing::{uri, TypedRouter};
use axum_typed_routing_macros::route;

/// This is a handler that is documented!
#[route(GET "/hello/{id}?user_id&name&opt_arg")]
async fn generic_handler_with_complex_options<T: 'static>(
    mut id: u32,
    user_id: String,
    name: String,
    opt_arg: Option<String>,
    State(state): State<String>,
    hello: State<String>,
    Json(mut json): Json<u32>,
) -> String {
    if let Some(opt_arg) = opt_arg {
        format!("Hello, {id} - {user_id} - {name} - {opt_arg}!")
    } else {
        format!("Hello, {id} - {user_id} - {name}!")
    }
}

#[route(POST "/one")]
async fn one(state: State<String>) -> String {
    String::from("Hello!")
}

#[route(POST "/two")]
async fn two() -> String {
    String::from("Hello!")
}

#[route(GET "/three/{id}")]
async fn three(id: u32) -> String {
    format!("Hello {id}!")
}

#[route(GET "/four?id")]
async fn four(id: u32) -> String {
    format!("Hello {id:?}!")
}

#[route(GET "/optional?id")]
async fn optional(id: Option<u32>) -> String {
    format!("Hello {id:?}!")
}

// Tests that hyphens are allowed in route names
#[route(GET "/foo-bar")]
async fn foo_bar() {}

#[tokio::test]
async fn test_normal() {
    let router: axum::Router = axum::Router::new()
        .typed_route(generic_handler_with_complex_options::<u32>)
        .typed_route(one)
        .with_state("state".to_string())
        .typed_route(two)
        .typed_route(three)
        .typed_route(four)
        .typed_route(optional);

    let server = TestServer::new(router).unwrap();

    let response = server.post("/one").await;
    response.assert_status_ok();
    response.assert_text("Hello!");

    let response = server.post("/two").await;
    response.assert_status_ok();
    response.assert_text("Hello!");

    let response = server.get("/three/123").await;
    response.assert_status_ok();
    response.assert_text("Hello 123!");

    let response = server.get("/four").add_query_param("id", 123).await;
    response.assert_status_ok();
    response.assert_text("Hello 123!");

    let response = server.get("/optional").add_query_param("id", 123).await;
    response.assert_status_ok();
    response.assert_text("Hello Some(123)!");

    let response = server.get("/optional").await;
    response.assert_status_ok();
    response.assert_text("Hello None!");

    let response = server
        .get("/hello/123")
        .add_query_param("user_id", 321.to_string())
        .add_query_param("name", "John".to_string())
        .json(&100)
        .await;
    response.assert_status_ok();
    response.assert_text("Hello, 123 - 321 - John!");

    let response = server
        .get("/hello/456")
        .add_query_param("user_id", 654.to_string())
        .add_query_param("name", "Peter".to_string())
        .add_query_param("opt_arg", "Opt".to_string())
        .json(&100)
        .await;
    response.assert_status_ok();
    response.assert_text("Hello, 456 - 654 - Peter - Opt!");

    let (path, method_router) = generic_handler_with_complex_options::<u32>();
    assert_eq!(path, "/hello/{id}");
}

#[route(GET "/*capture")]
async fn wildcard_capture(capture: String) -> Json<String> {
    Json(capture)
}

#[tokio::test]
async fn test_wildcard() {
    let router: axum::Router = axum::Router::new().typed_route(wildcard_capture);

    let server = TestServer::new(router).unwrap();

    let response = server.get("/foo/bar").await;
    response.assert_status_ok();
    assert_eq!(response.json::<String>(), "foo/bar");

    let response = server.get("/foo/bar").await;
    response.assert_status_ok();
    assert_eq!(response.json::<String>(), "foo/bar");
}

#[tokio::test]
async fn test_uri() {
    #[route(GET "/dog?name")]
    async fn dog(name: String) -> String {
        format!("Hello {name:?}!")
    }

    #[route(GET "/cat/{name}")]
    async fn cat(name: String) -> String {
        format!("Hello {name:?}!")
    }

    #[route(GET "/users/{user_id}/blogposts/{blogpost_id}")]
    async fn get_user_blogpost(user_id: u32, blogpost_id: u32) -> String {
        format!("blogpost {blogpost_id} for user {user_id}")
    }

    #[route(GET "/users/{user_id}/files/*path")]
    async fn get_user_file(user_id: u32, path: String) -> String {
        format!("file {path} for user {user_id}")
    }

    assert_eq!(uri!(one()), "/one");
    assert_eq!(uri!(two()), "/two");
    assert_eq!(uri!(three(id = 123)), "/three/123");
    assert_eq!(uri!(four(id = 123)), "/four?id=123");
    let id_var = 123;
    assert_eq!(uri!(four(id = id_var)), "/four?id=123");
    assert_eq!(
        uri!(get_user_blogpost(blogpost_id = 10, user_id = 5)),
        "/users/5/blogposts/10"
    );
    assert_eq!(
        uri!(get_user_file(
            user_id = 9,
            path = "my/file/lol.pdf".to_string()
        )),
        "/users/9/files/my/file/lol.pdf"
    );
    // This will (correctly) with a name mismatch:
    // assert_eq!(uri!(four(invalid = "should fail")), "/four?id=123");
    assert_eq!(uri!(optional(id = Some(123))), "/optional?id=123");
    assert_eq!(uri!(optional(id = _)), "/optional");
    assert_eq!(
        uri!(dog(name = "Foo Bar".to_string())),
        "/dog?name=Foo%20Bar"
    );
    assert_eq!(uri!(cat(name = "Foo Bar".to_string())), "/cat/Foo%20Bar");

    assert_eq!(
        uri!(wildcard_capture(capture = "something/else".to_string())),
        "/something/else"
    );

    assert_eq!(
        uri!(generic_handler_with_complex_options(
            id = 3,
            user_id = "Foo Bar".to_string(),
            name = "Robert".to_string(),
            opt_arg = _
        )),
        "/hello/3?user_id=Foo%20Bar&name=Robert"
    );
}

#[cfg(feature = "aide")]
mod aide_support {
    use super::*;
    use aide::{axum::ApiRouter, openapi::OpenApi, transform::TransformOperation};
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
            .typed_route(one)
            .typed_api_route(get_hello)
            .with_state("state".to_string());

        let (path, method_router) = get_hello();
        assert_eq!(path, "/hello");
        assert_eq!(uri!(get_hello()), "/hello");

        let (path, method_router) = post_hello();
        assert_eq!(path, "/hello");
        assert_eq!(uri!(get_hello()), "/hello");
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

    /// unused-summary
    ///
    /// unused-description
    #[api_route(GET "/hello" {
        summary: "MySummary",
        description: "MyDescription",
        hidden: false,
        id: "MyRoute",
        tags: ["MyTag1", "MyTag2"],
        security: {
            "MySecurity1": ["MyScope1", "MyScope2"],
            "MySecurity2": [],
        },
        responses: {
            300: String,
        },
        transform: |x| x.summary("OverriddenSummary"),
    })]
    async fn get_gello_with_attributes(state: State<String>) -> String {
        String::from("Hello!")
    }

    #[test]
    fn generated_from_attributes() {
        let router = ApiRouter::new().typed_api_route(get_gello_with_attributes);
        let mut api = OpenApi::default();
        router.finish_api(&mut api);

        let get_op = path_item(&api, "/hello").get.as_ref().unwrap();

        assert_eq!(get_op.summary, Some("OverriddenSummary".to_string()));
        assert_eq!(get_op.description, Some("MyDescription".to_string()));
        assert_eq!(
            get_op.tags,
            vec!["MyTag1".to_string(), "MyTag2".to_string()]
        );
        assert_eq!(get_op.operation_id, Some("MyRoute".to_string()));
    }

    /// summary
    ///
    /// description
    /// description
    #[api_route(GET "/hello")]
    async fn get_gello_without_attributes(state: State<String>) -> String {
        String::from("Hello!")
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
}
