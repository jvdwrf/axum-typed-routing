#![doc = include_str!("../../README.md")]
//!
//! ## Basic usage
//! The following example demonstrates the basic usage of the library.
//! On top of any regular handler, you can add the [`route`] macro to create a typed route.
//! Any path- or query-parameters in the url will be type-checked at compile-time, and properly
//! extracted into the handler.
//!
//! The following example shows how the path parameter `id`, and query parameters `amount` and
//! `offset` are type-checked and extracted into the handler.
//!
//! ```
#![doc = include_str!("../examples/basic.rs")]
//! ```
//!
//! Some valid url's as get-methods are:
//! - `/item/1?amount=2&offset=3`
//! - `/item/1?amount=2`
//! - `/item/1?offset=3`
//! - `/item/500`
//!
//! By marking the `amount` and `offset` parameters as `Option<T>`, they become optional.
//!
//! ## Example with `aide`
//! When the `aide` feature is enabled, it's possible to automatically generate OpenAPI
//! documentation for the routes. The [`api_route`] macro is used in place of the [`route`] macro.
//!
//! Please read the [`aide`] documentation for more information on usage.
//! ```
#![doc = include_str!("../examples/aide.rs")]
//! ```

use axum::routing::MethodRouter;

type TypedHandler<S = ()> = fn() -> (&'static str, MethodRouter<S>);
pub use axum_typed_routing_macros::{route, uri};

/// A trait that allows typed routes, created with the [`route`] macro to
/// be added to an axum router.
///
/// Typed handlers are of the form `fn() -> (&'static str, MethodRouter<S>)`, where
/// `S` is the state type. The first element of the tuple is the path, and the second
/// is the method router.
pub trait TypedRouter: Sized {
    /// The state type of the router.
    type State: Clone + Send + Sync + 'static;

    /// Add a typed route to the router, usually created with the [`route`] macro.
    ///
    /// Typed handlers are of the form `fn() -> (&'static str, MethodRouter<S>)`, where
    /// `S` is the state type. The first element of the tuple is the path, and the second
    /// is the method router.
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
        axum::{routing::ApiMethodRouter, ApiRouter},
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

    /// Same as [`TypedRouter`], but with support for `aide`.
    pub trait TypedApiRouter: TypedRouter {
        /// Same as [`TypedRouter::typed_route`], but with support for `aide`.
        fn typed_api_route(self, handler: TypedApiHandler<Self::State>) -> Self;

        /// Same as [`TypedApiRouter::typed_api_route`], but with a custom path transform for
        /// use with `aide`.
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

#[doc(hidden)]
pub mod __private {
    use percent_encoding::{utf8_percent_encode, AsciiSet, NON_ALPHANUMERIC};
    use std::fmt::Display;

    const RESERVED: &AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'.')
        .remove(b'_')
        .remove(b'~');
    const PATH_WILDCARD: &AsciiSet = &RESERVED.remove(b'/');

    /// Percent-encode a value for use in a URL query value or single path segment
    pub fn encode_uri_param<T: Display>(v: T) -> String {
        let s = v.to_string();
        utf8_percent_encode(&s, RESERVED).to_string()
    }

    /// Percent-encode a value for use as a multi-segment path (preserves `/`)
    ///
    /// As the name implies, this is used for properly handling wildcard paths.
    pub fn encode_uri_path_wildcard<T: Display>(v: T) -> String {
        let s = v.to_string();
        utf8_percent_encode(&s, PATH_WILDCARD).to_string()
    }
}
