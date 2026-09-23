//! Statically-typed routing macros for [axum], in the style of Rocket, with optional
//! OpenAPI generation through [aide](https://docs.rs/aide).
//!
//! Put the [`route`] attribute on a handler and write the path, including path and query
//! parameters, next to it. The macro checks at compile time that every parameter matches a
//! handler argument, and extracts the parameters into those arguments. Register the handler
//! with [`TypedRouter::typed_route`].
//!
//! # Dependencies
//! The generated code refers to some crates by name, so your crate needs them as direct
//! dependencies:
//! - always: `axum` and `serde` (with the `derive` feature);
//! - with the `aide` feature: also `aide` (with its `axum` and `axum-query` features) and
//!   `schemars`;
//! - for `debug` mode: axum's `macros` feature.
//!
//! # Basic usage
//! This handler takes the path parameter `id` and the query parameters `amount` and
//! `offset`, and registers it on a router:
//! ```
#![doc = include_str!("../examples/basic.rs")]
//! ```
//!
//! Some URLs this route matches:
//! - `/item/1?amount=2&offset=3`
//! - `/item/1?amount=2`
//! - `/item/1?offset=3`
//! - `/item/500`
//!
//! `amount` and `offset` have type `Option<u32>`, so they are optional. `State` and `Json`
//! are not named in the path, so they are passed through as ordinary axum extractors.
//!
//! The attribute replaces `item_handler` with a function
//! `fn() -> (&'static str, MethodRouter<S>)` that returns the path and axum's method router.
//! [`TypedRouter::typed_route`] calls it and adds the result to the router.
//!
//! # Path syntax
//! | Syntax           | Meaning                                                              |
//! |------------------|----------------------------------------------------------------------|
//! | `/item/{id}`     | Capture one segment into the argument `id`.                          |
//! | `/files/*path`   | Capture the rest of the path into `path`. It must be the last segment. |
//! | `?amount&offset` | Query parameters `amount` and `offset`. Use `Option<T>` for optional ones. |
//!
//! Each argument's type decides how its parameter is deserialized. The parameters are
//! collected into generated structs named `<Handler>Path` and `<Handler>Query`, which is
//! why `serde` must be a dependency. See [`route`] for the full syntax.
//!
//! # State
//! The state type is inferred from a `State<T>` argument, or is `()` if there is none. You can
//! also set it explicitly with `with`:
//! ```ignore
//! #[route(GET "/item/{id}" with AppState)]
//! ```
//!
//! # Generic handlers
//! Generic handlers stay generic. Pick the type parameters when you register them:
//! ```ignore
//! router.typed_route(handler::<u32>)
//! ```
//!
//! # Debug mode
//! Put `debug` before the method, as in `#[route(debug GET "/item/{id}")]`, to wrap the
//! handler in [`axum::debug_handler`][debug_handler], which gives clearer errors when an
//! extractor or the return type is wrong. This requires axum's `macros` feature. With
//! `api_route`, `debug` also checks that every extractor implements `aide::OperationInput`
//! and that the return type implements `aide::OperationOutput`.
//!
//! [debug_handler]: https://docs.rs/axum/latest/axum/attr.debug_handler.html
//!
//! # OpenAPI with `aide`
//! With the `aide` feature enabled, use the [`api_route`] macro instead of [`route`] and
//! register routes on an [`aide::axum::ApiRouter`] with [`TypedApiRouter::typed_api_route`].
//!
//! The operation is filled in from the handler:
//! - the first line of the doc comment becomes the summary;
//! - the rest of the doc comment, after the blank line, becomes the description;
//! - doc comments on path and query arguments become parameter descriptions;
//! - the function name becomes the `operationId`.
//!
//! You can override any of these, and set tags, security, responses and more. See
//! [`api_route`] for all the options.
//!
//! To document error responses too, see the companion crate
//! [axum-error-sets](https://docs.rs/axum-error-sets). It lets a handler declare the exact
//! set of status codes it can return, and each one shows up in the OpenAPI documentation.
#![cfg_attr(
    feature = "aide",
    doc = concat!("```\n", include_str!("../examples/aide.rs"), "\n```")
)]
//!
//! # Feature flags
//! - `aide`: enables [`api_route`], [`TypedApiRouter`], and [`TypedRouter`] for
//!   [`aide::axum::ApiRouter`].

use axum::routing::MethodRouter;

type TypedHandler<S = ()> = fn() -> (&'static str, MethodRouter<S>);
pub use axum_typed_routing_macros::route;

/// Adds typed routes, created with the [`route`] macro, to a router.
///
/// It is implemented for [`axum::Router`], and for `aide::axum::ApiRouter` when the `aide`
/// feature is enabled.
///
/// A typed handler is a function `fn() -> (&'static str, MethodRouter<S>)`, where `S` is the
/// state type. It returns the route's path and its method router.
///
/// ```
/// use axum_typed_routing::{TypedRouter, route};
///
/// #[route(GET "/hello/{name}")]
/// async fn hello(name: String) -> String {
///     format!("Hello, {name}!")
/// }
///
/// let router: axum::Router = axum::Router::new().typed_route(hello);
/// ```
pub trait TypedRouter: Sized {
    /// The state type of the router.
    type State: Clone + Send + Sync + 'static;

    /// Adds a typed route, usually created with the [`route`] macro, to the router.
    ///
    /// This is the same as `router.route(path, method_router)` with the tuple the handler
    /// returns.
    fn typed_route(self, handler: TypedHandler<Self::State>) -> Self;
}

impl<S> TypedRouter for axum::Router<S>
where
    S: Send + Sync + Clone + 'static,
{
    type State = S;

    fn typed_route(self, handler: TypedHandler<Self::State>) -> Self {
        let (path, method_router) = handler();
        self.route(path, method_router)
    }
}

#[cfg(feature = "aide")]
pub use aide_support::*;
#[cfg(feature = "aide")]
mod aide_support {
    use crate::{TypedHandler, TypedRouter};
    use aide::{
        axum::{ApiRouter, routing::ApiMethodRouter},
        transform::TransformPathItem,
    };

    type TypedApiHandler<S = ()> = fn() -> (&'static str, ApiMethodRouter<S>);

    pub use axum_typed_routing_macros::api_route;

    impl<S> TypedRouter for ApiRouter<S>
    where
        S: Send + Sync + Clone + 'static,
    {
        type State = S;

        fn typed_route(self, handler: TypedHandler<Self::State>) -> Self {
            let (path, method_router) = handler();
            self.route(path, method_router)
        }
    }

    /// Adds typed routes, created with the [`api_route`] macro, to an [`ApiRouter`] so that
    /// they appear in the generated OpenAPI documentation.
    ///
    /// Routes created with [`route`](crate::route) can still be added to an [`ApiRouter`] with
    /// [`TypedRouter::typed_route`]. They work, but are left out of the documentation.
    ///
    /// ```
    /// use aide::{axum::ApiRouter, openapi::OpenApi};
    /// use axum_typed_routing::{TypedApiRouter, api_route};
    ///
    /// /// Say hello
    /// #[api_route(GET "/hello/{name}")]
    /// async fn hello(name: String) -> String {
    ///     format!("Hello, {name}!")
    /// }
    ///
    /// let mut api = OpenApi::default();
    /// let router: axum::Router = ApiRouter::new()
    ///     .typed_api_route(hello)
    ///     .finish_api(&mut api);
    /// ```
    pub trait TypedApiRouter: TypedRouter {
        /// Adds a typed route, created with the [`api_route`] macro, to the router, and
        /// includes it in the OpenAPI documentation.
        fn typed_api_route(self, handler: TypedApiHandler<Self::State>) -> Self;

        /// Same as [`TypedApiRouter::typed_api_route`], but also applies `transform` to the
        /// path item's documentation. See [`ApiRouter::api_route_with`].
        fn typed_api_route_with(
            self,
            handler: TypedApiHandler<Self::State>,
            transform: impl FnOnce(TransformPathItem) -> TransformPathItem,
        ) -> Self;
    }

    impl<S> TypedApiRouter for ApiRouter<S>
    where
        S: Send + Sync + Clone + 'static,
    {
        fn typed_api_route(self, handler: TypedApiHandler<Self::State>) -> Self {
            let (path, method_router) = handler();
            self.api_route(path, method_router)
        }

        fn typed_api_route_with(
            self,
            handler: TypedApiHandler<Self::State>,
            transform: impl FnOnce(TransformPathItem) -> TransformPathItem,
        ) -> Self {
            let (path, method_router) = handler();
            self.api_route_with(path, method_router, transform)
        }
    }
}
