use compilation::CompiledRoute;
use parsing::{Method, Route};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use std::collections::HashMap;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Colon, Comma, Slash},
    FnArg, GenericArgument, ItemFn, LitStr, Meta, PathArguments, Signature, Type,
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
    match _route(attr, item.clone(), false) {
        Ok(tokens) => tokens.into(),
        Err(err) => {
            let err: TokenStream = err.to_compile_error().into();
            item.extend(err);
            item
        }
    }
}

/// Same as [`macro@route`], but with support for OpenApi using `aide`. See [`macro@route`] for more
/// information and examples.
///
///
/// # Syntax
/// ```ignore
/// #[api_route(<METHOD> "<PATH>" [with <STATE>] [{
///     summary = "<SUMMARY>",
///     description = "<DESCRIPTION>",
///     id = "<ID>",
///     tags = ["<TAG>", ..],
///     hidden = <bool>,
///     transform = |op| { .. },
/// }])]
/// ```
/// - `summary` is the OpenApi summary. If not specified, the first line of the function's doc-comments
/// - `description` is the OpenApi description. If not specified, the rest of the function's doc-comments
/// - `id` is the OpenApi operationId. If not specified, the function's name is used.
/// - `tags` are the OpenApi tags.
/// - `hidden` sets whether docs should be hidden for this route.
/// - `transform` is a closure that takes an `TransformOperation` and returns an `TransformOperation`.
/// This may override the other options. (see the crate `aide` for more information).
#[proc_macro_attribute]
pub fn api_route(attr: TokenStream, mut item: TokenStream) -> TokenStream {
    match _route(attr, item.clone(), true) {
        Ok(tokens) => tokens.into(),
        Err(err) => {
            let err: TokenStream = err.to_compile_error().into();
            item.extend(err);
            item
        }
    }
}

fn _route(attr: TokenStream, item: TokenStream, with_aide: bool) -> syn::Result<TokenStream2> {
    // Parse the route and function
    let route = syn::parse::<Route>(attr)?;
    let function = syn::parse::<ItemFn>(item)?;

    // Now we can compile the route
    let route = CompiledRoute::from_route(route, &function.sig, with_aide)?;
    let path_extractor = route.path_extractor();
    let query_extractor = route.query_extractor();
    let state_type = &route.state;
    let axum_path = route.to_axum_path_string();
    let http_method = route.method.to_axum_method_name();
    let remaining_numbered_pats = route.remaining_pattypes_numbered(&function.sig.inputs);
    let extracted_idents = route.extracted_idents();
    let remaining_numbered_idents = remaining_numbered_pats.iter().map(|pat_type| &pat_type.pat);
    let route_docs = route.to_doc_comments(&function.sig);

    // Get the variables we need for code generation
    let fn_name = &function.sig.ident;
    let fn_output = &function.sig.output;
    let vis = &function.vis;
    let asyncness = &function.sig.asyncness;
    let (impl_generics, ty_generics, where_clause) = &function.sig.generics.split_for_impl();
    let ty_generics = ty_generics.as_turbofish();
    let fn_docs = function
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"));

    let (inner_fn_call, method_router_ty) = match with_aide {
        true => {
            let http_method = format_ident!("{}_with", http_method);
            let summary = route
                .get_oapi_summary(&function.attrs)
                .map(|summary| quote! { .summary(#summary) });
            let description = route
                .get_oapi_description(&function.attrs)
                .map(|description| quote! { .description(#description) });
            let hidden = route
                .get_oapi_hidden()
                .map(|hidden| quote! { .hidden(#hidden) });
            let tags = route.get_oapi_tags();
            let id = route
                .get_oapi_id(&function.sig)
                .map(|id| quote! { .id(#id) });
            let transform = route.get_oapi_transform()?;
            (
                quote! {
                    ::aide::axum::routing::#http_method(
                        __inner__function__ #ty_generics,
                        |__op__| {
                            let __op__ = __op__
                                #summary
                                #description
                                #hidden
                                #id
                                #(.tag(#tags))*;
                            #transform
                            __op__
                        }
                    )
                },
                quote! { ::aide::axum::routing::ApiMethodRouter },
            )
        }
        false => (
            quote! { ::axum::routing::#http_method(__inner__function__ #ty_generics) },
            quote! { ::axum::routing::MethodRouter },
        ),
    };

    // Generate the code
    Ok(quote! {
        #(#fn_docs)*
        #route_docs
        #vis fn #fn_name #impl_generics() -> (&'static str, #method_router_ty<#state_type>) #where_clause {

            #asyncness fn __inner__function__ #impl_generics(
                #path_extractor
                #query_extractor
                #remaining_numbered_pats
            ) #fn_output #where_clause {
                #function

                #fn_name #ty_generics(#(#extracted_idents,)* #(#remaining_numbered_idents,)* ).await
            }

            (#axum_path, #inner_fn_call)
        }
    })
}
