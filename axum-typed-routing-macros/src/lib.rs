use compilation::CompiledRoute;
use parsing::{Method, Route};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use std::collections::HashMap;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Colon, Comma, Slash},
    FnArg, GenericArgument, ItemFn, LitStr, Path, PathArguments, Signature, Type,
};
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

mod compilation;
mod parsing;

/// A macro that generates statically-typed routes for axum handlers.
///
/// # Syntax
/// ```ignore
/// #[route(<METHOD> "<PATH>" [with <STATE>])]
/// ```
/// - `METHOD` is the HTTP method, such as `GET`, `POST`, `PUT`, etc.
/// - `PATH` is the path of the route, with optional path parameters and query parameters,
///     e.g. `/item/:id?amount&offset`.
/// - `STATE` is the type of axum-state, passed to the handler. This is optional, and if not
///    specified, the state type is guessed based on the parameters of the handler.
///
/// # Example
/// ```ignore
/// use axum::extract::{State, Json};
/// use axum_typed_routing::route;
///
/// #[route(GET "/item/:id?amount&offset")]
/// async fn item_handler(
///     id: u32,
///     amount: Option<u32>,
///     offset: Option<u32>,
///     State(state): State<String>,
///     Json(json): Json<u32>,
/// ) -> String {
///     todo!("handle request")
/// }
/// ```
///
/// # State type
/// Normally, the state-type is guessed based on the parameters of the function:
/// If the function has a parameter of type `[..]::State<T>`, then `T` is used as the state type.
/// This should work for most cases, however when not sufficient, the state type can be specified
/// explicitly using the `with` keyword:
/// ```ignore
/// #[route(GET "/item/:id?amount&offset" with String)]
/// ```
///
/// # Internals
/// The macro expands to a function with signature `fn() -> (&'static str, axum::routing::MethodRouter<S>)`.
/// The first element of the tuple is the path, and the second is axum's `MethodRouter`.
///
/// The path and query are extracted using axum's `extract::Path` and `extract::Query` extractors, as the first
/// and second parameters of the function. The remaining parameters are the parameters of the handler.
#[proc_macro_attribute]
pub fn route(attr: TokenStream, mut item: TokenStream) -> TokenStream {
    match _route(
        attr,
        item.clone(),
        parse_quote!(::axum::routing),
        parse_quote!(MethodRouter),
    ) {
        Ok(tokens) => tokens.into(),
        Err(err) => {
            let err: TokenStream = err.to_compile_error().into();
            item.extend(err);
            item
        }
    }
}

/// Same as [`macro@route`], but with support for `aide`.
#[proc_macro_attribute]
pub fn api_route(attr: TokenStream, mut item: TokenStream) -> TokenStream {
    match _route(
        attr,
        item.clone(),
        parse_quote!(::aide::axum::routing),
        parse_quote!(ApiMethodRouter),
    ) {
        Ok(tokens) => tokens.into(),
        Err(err) => {
            let err: TokenStream = err.to_compile_error().into();
            item.extend(err);
            item
        }
    }
}

fn _route(
    attr: TokenStream,
    item: TokenStream,
    routing_prefix: Path,
    method_router: Ident,
) -> syn::Result<TokenStream2> {
    // Parse the route and function
    let route = syn::parse::<Route>(attr)?;
    let function = syn::parse::<ItemFn>(item)?;

    // Now we can compile the route
    let route = CompiledRoute::from_route(route, &function.sig)?;
    let path_extractor = route.path_extractor();
    let query_extractor = route.query_extractor();
    let state_type = &route.state;
    let axum_path = route.to_axum_path_string();
    let http_method = route.method.to_axum_method_name();
    let remaining_args = route.remaining_args(&function.sig.inputs);
    let route_docs = route.to_doc_comments(&function.sig);

    // Get the variables we need for code generation
    let name = &function.sig.ident;
    let output = &function.sig.output;
    let vis = &function.vis;
    let asyncness = &function.sig.asyncness;
    let (impl_generics, ty_generics, where_clause) = &function.sig.generics.split_for_impl();
    let ty_generics = ty_generics.as_turbofish();
    let pats = function.sig.inputs.iter().map(|arg| match arg {
        FnArg::Receiver(_) => unimplemented!("`self` arguments are not supported"),
        FnArg::Typed(pat_type) => {
            let pat = &pat_type.pat;
            quote! { #pat }
        }
    });
    let docs = function
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .collect::<Vec<_>>();

    // Generate the code
    Ok(quote! {
        #(#docs)*
        #route_docs
        #vis fn #name #impl_generics() -> (&'static str, #routing_prefix::#method_router<#state_type>) #where_clause {

            #asyncness fn #name #impl_generics(
                #path_extractor
                #query_extractor
                #remaining_args
            ) #output #where_clause {
                #function

                #name #ty_generics(#(#pats),*).await
            }

            (#axum_path, #routing_prefix::#http_method(#name #ty_generics))
        }
    })
}
