use compilation::CompiledRoute;
use parsing::{Method, Route};
use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use std::collections::HashMap;
use syn::{
    FnArg, GenericArgument, ItemFn, LitStr, Meta, PathArguments, Signature, Type,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::{Comma, Slash},
};
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

mod compilation;
mod parsing;

/// Turns an axum handler into a statically-typed route.
///
/// The path and query parameters named in the route are checked against the handler's arguments
/// at compile time, and extracted into them.
///
/// # Syntax
/// ```ignore
/// #[route([debug] <METHOD> "<PATH>" [with <STATE>])]
/// ```
/// - `debug` (optional) adds [`#[axum::debug_handler]`](https://docs.rs/axum/latest/axum/attr.debug_handler.html)
///   to the handler for better compiler errors. This requires axum's `macros` feature.
/// - `METHOD` is the HTTP method: `GET`, `POST`, `PUT`, `PATCH`, `DELETE`, `HEAD`, `CONNECT`,
///   `OPTIONS` or `TRACE`. It is case-insensitive.
/// - `PATH` is the route's path, with optional path parameters and query parameters, for
///   example `/item/{id}?amount&offset`. See [Path syntax](#path-syntax).
/// - `STATE` (optional) is the axum state type. If you leave it out, it is inferred from the
///   handler's arguments. See [State type](#state-type).
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
/// # Path syntax
/// - `/item/{id}` captures one path segment into the argument `id`.
/// - `/files/*path` captures the rest of the path into the argument `path`. A wildcard must be
///   the last segment. It becomes axum's `{*path}` syntax.
/// - `?amount&offset` declares query parameters `amount` and `offset`. Use `Option<T>` for an
///   optional query parameter.
///
/// Every path and query parameter must have a handler argument with the same name, or
/// compilation fails. That argument's type decides how the parameter is deserialized. All other
/// arguments are passed through as ordinary axum extractors, in order.
///
/// Path and query parameters are deserialized into generated structs named `<Handler>Path` and
/// `<Handler>Query` (for example `ItemHandlerPath` and `ItemHandlerQuery`), which derive
/// `serde::Deserialize`. **Your crate therefore needs `serde` as a direct dependency.**
///
/// Doc comments on arguments are allowed. They are moved onto the fields of the generated
/// structs, where [`macro@api_route`] uses them as parameter descriptions in the OpenAPI schema.
///
/// # State type
/// By default the state type is inferred from the arguments: if an argument has type
/// `State<T>` (with any path prefix), `T` is used. If there is none, the state is `()`.
/// When that isn't enough, for example because the state is only used by a custom extractor,
/// set it explicitly with `with`:
/// ```ignore
/// #[route(GET "/item/{id}?amount&offset" with String)]
/// ```
///
/// # Expansion
/// The attribute **replaces** the handler with a function of the same name, visibility and
/// generics, with signature
/// ```ignore
/// fn() -> (&'static str, axum::routing::MethodRouter<S>)
/// ```
/// It returns the axum path (for example `"/item/{id}"`) and the method router. You can no
/// longer call it as a handler directly. Add it to a router with
/// [`TypedRouter::typed_route`](https://docs.rs/axum-typed-routing/latest/axum_typed_routing/trait.TypedRouter.html#tymethod.typed_route),
/// or destructure the tuple yourself. Generic handlers are registered with a turbofish:
/// `router.typed_route(handler::<u32>)`.
///
/// The handler's own doc comments are kept, and a short summary of the route (method, path,
/// state) is appended to them.
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

/// Same as [`macro@route`], but also generates OpenAPI documentation with
/// [`aide`](https://docs.rs/aide).
///
/// The generated function returns an `aide::axum::routing::ApiMethodRouter` instead of axum's
/// `MethodRouter`. Add it to an `aide::axum::ApiRouter` with
/// [`TypedApiRouter::typed_api_route`](https://docs.rs/axum-typed-routing/latest/axum_typed_routing/trait.TypedApiRouter.html#tymethod.typed_api_route).
/// Everything described for [`macro@route`] applies here too.
///
/// Your crate needs `serde`, `schemars` and `aide` as direct dependencies, because the
/// generated parameter structs derive `serde::Deserialize` and `schemars::JsonSchema`.
///
/// # Syntax
/// ```ignore
/// #[api_route([debug] <METHOD> "<PATH>" [with <STATE>] [{
///     summary: "<SUMMARY>",
///     description: "<DESCRIPTION>",
///     id: "<ID>",
///     tags: ["<TAG>", ..],
///     hidden: <bool>,
///     security: { "<SCHEME>": ["<SCOPE>", ..], .. },
///     responses: { <CODE>: <TYPE>, .. },
///     transform: |op| { .. },
/// }])]
/// ```
/// Every option is optional:
/// - `summary`: the operation summary. Defaults to the first line of the handler's doc comment.
/// - `description`: the operation description. Defaults to the handler's doc comment after the
///   first line and the blank line that follows it.
/// - `id`: the `operationId`. Defaults to the handler's name.
/// - `tags`: the operation's tags.
/// - `hidden`: whether to hide the operation from the generated documentation.
/// - `security`: security requirements, as a map from scheme name to required scopes.
/// - `responses`: extra responses, as a map from status code to response type. Each type
///   must implement `aide::OperationOutput`.
/// - `transform`: a closure `|op| ..` that receives the `aide::transform::TransformOperation`
///   and returns it. It runs after the other options, so it can override them.
///
/// Doc comments on path and query parameter arguments become the parameters' descriptions.
///
/// With `debug`, the macro also checks at compile time that every extractor argument
/// implements `aide::OperationInput` and that the return type implements
/// `aide::OperationOutput`, so a missing implementation gives a clear error at the handler.
///
/// # Example
/// ```
/// use axum::extract::{State, Json};
/// use axum_typed_routing_macros::api_route;
///
/// /// Get an item
/// ///
/// /// Returns the item with the given id.
/// #[api_route(GET "/item/{id}?amount&offset" with String {
///     id: "get-item",
///     tags: ["items"],
///     hidden: false,
///     security: { "bearer": ["read:items"] },
///     responses: { 200: String },
///     transform: |op| op.tag("private"),
/// })]
/// async fn item_handler(
///     /// The id of the item
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

    let mut function = syn::parse::<ItemFn>(item)?;

    // Now we can compile the route
    let route = CompiledRoute::from_route(route, &mut function, with_aide)?;
    let (path_extractor, _path_ty) = route.path_extractor();
    let (query_extractor, _query_ty) = route.query_extractor();
    let query_params_struct = route.query_params_struct(with_aide);
    let path_params_struct = route.path_params_struct(with_aide);
    let state_type = &route.state;
    let axum_path = route.to_axum_path_string();
    let http_method = route.method.to_axum_method_name();
    let remaining_numbered_pats = route.remaining_pattypes_numbered(&function.sig.inputs);
    let extracted_idents = route.extracted_idents();
    let remaining_numbered_idents = remaining_numbered_pats.iter().map(|pat_type| &pat_type.pat);
    let route_docs = route.to_doc_comments();

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
    let debug_handler = route.axum_debug_handler();
    let debug_operation_input_output = route.debug_operation_input_output(&function, with_aide);

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
                            // .input::<#path_ty>()
                            // .input::<#query_ty>()
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
            #path_params_struct

            #debug_operation_input_output

            #aide_ident_docs
            #debug_handler
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
