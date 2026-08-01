use std::collections::HashMap;

use compilation::CompiledRoute;
use parsing::{Method, Route, Uri};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Comma, Slash},
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
///     e.g. `/item/{id}?amount&offset`.
/// - `STATE` is the type of axum-state, passed to the handler. This is optional, and if not
///    specified, the state type is guessed based on the parameters of the handler.
///
/// # Example
/// ```
/// use axum::extract::{State, Json};
/// use axum_typed_routing_macros::route;
///
/// #[route(GET "/item/{id}?amount&offset")]
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
/// #[route(GET "/item/{id}?amount&offset" with String)]
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
/// # Syntax
/// ```ignore
/// #[api_route(<METHOD> "<PATH>" [with <STATE>] [{
///     summary: "<SUMMARY>",
///     description: "<DESCRIPTION>",
///     id: "<ID>",
///     tags: ["<TAG>", ..],
///     hidden: <bool>,
///     security: { <SCHEME>: ["<SCOPE>", ..], .. },
///     responses: { <CODE>: <TYPE>, .. },
///     transform: |op| { .. },
/// }])]
/// ```
/// - `summary` is the OpenApi summary. If not specified, the first line of the function's doc-comments
/// - `description` is the OpenApi description. If not specified, the rest of the function's doc-comments
/// - `id` is the OpenApi operationId. If not specified, the function's name is used.
/// - `tags` are the OpenApi tags.
/// - `hidden` sets whether docs should be hidden for this route.
/// - `security` is the OpenApi security requirements.
/// - `responses` are the OpenApi responses.
/// - `transform` is a closure that takes an `TransformOperation` and returns an `TransformOperation`.
/// This may override the other options. (see the crate `aide` for more information).
///
/// # Example
/// ```
/// use axum::extract::{State, Json};
/// use axum_typed_routing_macros::api_route;
///
/// #[api_route(GET "/item/{id}?amount&offset" with String {
///     summary: "Get an item",
///     description: "Get an item by id",
///     id: "get-item",
///     tags: ["items"],
///     hidden: false,
///     security: { "bearer": ["read:items"] },
///     responses: { 200: String },
///     transform: |op| op.tag("private"),
/// })]
/// async fn item_handler(
///     id: u32,
///     amount: Option<u32>,
///     offset: Option<u32>,
///     State(state): State<String>,
/// ) -> String {
///     todo!("handle request")
/// }
/// ```
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
    let route = CompiledRoute::from_route(route, &function, with_aide)?;
    let path_extractor = route.path_extractor();
    let query_extractor = route.query_extractor();
    let query_params_struct = route.query_params_struct(with_aide);
    let state_type = &route.state;
    let axum_path = route.to_axum_path_string();
    let route_format = route.to_path_format_string();
    let http_method = route.method.to_axum_method_name();
    let remaining_numbered_pats = route.remaining_pattypes_numbered(&function.sig.inputs);
    let (query_idents, query_types) = route.query_idents_and_types();
    let (path_idents, path_types) = route.path_idents_and_types();
    let path_and_query_idents = [path_idents, query_idents].concat();
    let path_and_query_types = [path_types, query_types].concat();
    let path_encode_calls = route.path_encode_calls();
    let query_snippets = route.query_snippets();

    let remaining_numbered_idents = remaining_numbered_pats.iter().map(|pat_type| &pat_type.pat);
    let route_docs = route.to_doc_comments();

    // Get the variables we need for code generation
    let fn_name = &function.sig.ident;
    let fn_name_uri_mod_name = format_ident!("{}_uri_internal", fn_name);
    let fn_output = &function.sig.output;
    let vis = &function.vis;
    let asyncness = &function.sig.asyncness;
    let (impl_generics, ty_generics, where_clause) = &function.sig.generics.split_for_impl();
    let ty_generics = ty_generics.as_turbofish();
    let fn_docs = function
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"));

    let (aide_ident_docs, inner_fn_call, method_router_ty) = if with_aide {
        let http_method = format_ident!("{}_with", http_method);
        let summary = route
            .get_oapi_summary()
            .map(|summary| quote! { .summary(#summary) });
        let description = route
            .get_oapi_description()
            .map(|description| quote! { .description(#description) });
        let hidden = route
            .get_oapi_hidden()
            .map(|hidden| quote! { .hidden(#hidden) });
        let tags = route.get_oapi_tags();
        let id = route
            .get_oapi_id(&function.sig)
            .map(|id| quote! { .id(#id) });
        let transform = route.get_oapi_transform()?;
        let responses = route.get_oapi_responses();
        let response_code = responses.iter().map(|response| &response.0);
        let response_type = responses.iter().map(|response| &response.1);
        let security = route.get_oapi_security();
        let schemes = security.iter().map(|sec| &sec.0);
        let scopes = security.iter().map(|sec| &sec.1);

        (
            route.ide_documentation_for_aide_methods(),
            quote! {
                ::aide::axum::routing::#http_method(
                    __inner__function__ #ty_generics,
                    |__op__| {
                        let __op__ = __op__
                            #summary
                            #description
                            #hidden
                            #id
                            #(.tag(#tags))*
                            #(.security_requirement_scopes::<Vec<&'static str>, _>(#schemes, vec![#(#scopes),*]))*
                            #(.response::<#response_code, #response_type>())*
                            ;
                        #transform
                        __op__
                    }
                )
            },
            quote! { ::aide::axum::routing::ApiMethodRouter },
        )
    } else {
        (
            quote!(),
            quote! { ::axum::routing::#http_method(__inner__function__ #ty_generics) },
            quote! { ::axum::routing::MethodRouter },
        )
    };

    // Generate the code
    Ok(quote! {
        #(#fn_docs)*
        #route_docs
        #vis fn #fn_name #impl_generics() -> (&'static str, #method_router_ty<#state_type>) #where_clause {

            #query_params_struct

            #aide_ident_docs
            #asyncness fn __inner__function__ #impl_generics(
                #path_extractor
                #query_extractor
                #remaining_numbered_pats
            ) #fn_output #where_clause {
                #function

                #fn_name #ty_generics(#(#path_and_query_idents,)* #(#remaining_numbered_idents,)* ).await
            }

            (#axum_path, #inner_fn_call)
        }

        #[doc(hidden)]
        #[allow(non_snake_case)]
        #vis mod #fn_name_uri_mod_name {
            use super::*;

            #[allow(non_camel_case_types)]
            pub struct Args {
                #(pub #path_and_query_idents: #path_and_query_types,)*
            }

            #[doc(hidden)]
            #[allow(unused_assignments)]
            pub fn uri(args: Args) -> String {
                let Args { #(#path_and_query_idents,)* } = args;
                let mut __atr_uri = format!(#route_format, #(#path_encode_calls,)*);
                let mut __atr_sep = '?';
                #(#query_snippets)*
                __atr_uri
            }
        }
    })
}

/// Generate a type-safe, encoded route URI
///
/// It handles dynamic route parameters, query parameters as well as optional query parameters.
///
/// # Examples
/// ```
/// use axum_typed_routing_macros::{route, uri};
///
/// // Simple case with route parameter
/// #[route(GET "/dog/{name}")]
/// async fn dog(name: String) -> String {
///     format!("Hello {name}!")
/// }
/// let dog = uri!(dog(name = "Foo Bar".to_string()));
/// // Notice that the space is safely encoded.
/// assert_eq!(dog, "/dog/Foo%20Bar");
///
/// // Simple case with query parameter
/// #[route(GET "/users?id")]
/// async fn users(id: Option<u32>) -> String {
///     format!("This is user {id:?}!")
/// }
/// let users = uri!(users(id = Some(5)));
/// assert_eq!(users, "/users?id=5");
///
/// // For optional parameters, you can set `None` by passing `_` like so:
/// let users = uri!(users(id = _));
/// assert_eq!(users, "/users");
/// ```
#[proc_macro]
pub fn uri(input: TokenStream) -> TokenStream {
    match _uri(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => {
            let err: TokenStream = err.to_compile_error().into();
            err
        }
    }
}

fn _uri(input: TokenStream) -> syn::Result<TokenStream2> {
    let input = syn::parse::<Uri>(input)?;
    let fields = input.params.iter().map(|(name, v)| {
        let v = match v {
            parsing::UriValue::Skip => quote! { None },
            parsing::UriValue::Expr(expr) => quote! { #expr },
        };
        quote! { #name: #v }
    });

    let fn_name_uri_mod_name = format_ident!("{}_uri_internal", input.route_name);

    Ok(quote! {
        #fn_name_uri_mod_name::uri(#fn_name_uri_mod_name::Args { #(#fields),* })
    })
}
