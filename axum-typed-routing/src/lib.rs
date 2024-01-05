#![doc = include_str!("../../README.md")]
use axum::routing::MethodRouter;

type TypedHandler<S = ()> = fn() -> (&'static str, MethodRouter<S>);
pub use axum_typed_routing_macros::route;

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
    use aide::axum::{routing::ApiMethodRouter, ApiRouter};

    type TypedApiHandler<S = ()> = fn() -> (&'static str, ApiMethodRouter<S>);

    pub use axum_typed_routing_macros::api_route;

    /// Same as [`TypedRouter`], but with support for `aide`.
    pub trait TypedApiRouter: TypedRouter {
        /// Same as [`TypedRouter::typed_route`], but with support for `aide`.
        fn typed_api_route(self, handler: TypedApiHandler<Self::State>) -> Self;
    }

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

    impl<S> TypedApiRouter for ApiRouter<S>
    where
        S: Send + Sync + Clone + 'static,
    {
        fn typed_api_route(self, handler: TypedApiHandler<Self::State>) -> Self {
            let (path, method_router) = handler();
            self.api_route(path, method_router)
        }
    }
}
