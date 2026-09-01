use convert_case::{Case, Casing as _};
use quote::ToTokens;
use syn::{Attribute, LitBool, LitInt, Pat, PatType, ReturnType, spanned::Spanned};

use crate::parsing::{OapiOptions, Responses, Security, StrArray};

use self::parsing::PathParam;

use super::*;

pub struct CompiledRoute {
    pub method: Method,
    #[allow(clippy::type_complexity)]
    pub path_params: Vec<(Slash, PathParam, Vec<Attribute>)>,
    pub query_params: Vec<(Ident, Box<Type>, Vec<Attribute>)>,
    pub state: Type,
    pub route_lit: LitStr,
    pub oapi_options: Option<OapiOptions>,
    pub fn_name: Ident,
    pub debug: bool,
}

impl CompiledRoute {
    pub fn to_axum_path_string(&self) -> String {
        let mut path = String::new();

        for (_slash, param, _attr) in &self.path_params {
            path.push('/');
            match param {
                PathParam::Capture(lit, _brace_1, _, _, _brace_2) => {
                    path.push('{');
                    path.push_str(&lit.value());
                    path.push('}');
                }
                PathParam::WildCard(lit, _brace_1, _, _, _, _brace_2) => {
                    path.push('{');
                    path.push('*');
                    path.push_str(&lit.value());
                    path.push('}');
                }
                PathParam::Static(lit) => path.push_str(&lit.value()),
            }
            // if colon.is_some() {
            //     path.push(':');
            // }
            // path.push_str(&ident.value());
        }

        path
    }

    /// Removes the arguments in `route` from `args`, and merges them in the output.
    pub fn from_route(
        mut route: Route,
        function: &mut ItemFn,
        with_aide: bool,
    ) -> syn::Result<Self> {
        if !with_aide && route.oapi_options.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "Use `api_route` instead of `route` to use OpenAPI options",
            ));
        } else if with_aide && route.oapi_options.is_none() {
            route.oapi_options = Some(OapiOptions {
                summary: None,
                description: None,
                id: None,
                hidden: None,
                tags: None,
                security: None,
                responses: None,
                transform: None,
            });
        }

        let mut arg_map = function
            .sig
            .inputs
            .iter_mut()
            .filter_map(|item| match item {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(pat_type) => Some(pat_type),
            })
            .filter_map(|pat_type| {
                let doc = pat_type
                    .attrs
                    .iter()
                    .filter(|attr| attr.path().is_ident("doc"))
                    .cloned()
                    .collect::<Vec<_>>();
                pat_type.attrs.retain(|attr| !attr.path().is_ident("doc"));
                match &*pat_type.pat {
                    syn::Pat::Ident(ident) => {
                        Some((ident.ident.clone(), (pat_type.ty.clone(), doc)))
                    }
                    _ => None,
                }
            })
            .collect::<HashMap<_, _>>();

        let mut path_params = Vec::new();
        for (slash, mut path_param) in route.path_params {
            let doc: Vec<Attribute>;

            match &mut path_param {
                PathParam::Capture(_lit, _, ident, ty, _) => {
                    let (new_ident, (new_ty, new_doc)) =
                        arg_map.remove_entry(ident).ok_or_else(|| {
                            syn::Error::new(
                                ident.span(),
                                format!(
                                    "path parameter `{}` not found in function arguments",
                                    ident
                                ),
                            )
                        })?;
                    *ident = new_ident;
                    *ty = new_ty;
                    doc = new_doc;
                }
                PathParam::WildCard(_lit, _, _star, ident, ty, _) => {
                    let (new_ident, (new_ty, new_doc)) =
                        arg_map.remove_entry(ident).ok_or_else(|| {
                            syn::Error::new(
                                ident.span(),
                                format!(
                                    "path parameter `{}` not found in function arguments",
                                    ident
                                ),
                            )
                        })?;
                    *ident = new_ident;
                    *ty = new_ty;
                    doc = new_doc;
                }
                PathParam::Static(_lit) => {
                    doc = Vec::new();
                }
            }

            path_params.push((slash, path_param, doc));
        }

        let mut query_params = Vec::new();
        for ident in route.query_params {
            let (ident, (ty, doc)) = arg_map.remove_entry(&ident).ok_or_else(|| {
                syn::Error::new(
                    ident.span(),
                    format!(
                        "query parameter `{}` not found in function arguments",
                        ident
                    ),
                )
            })?;
            query_params.push((ident, ty, doc));
        }

        if let Some(options) = route.oapi_options.as_mut() {
            options.merge_with_fn(function)
        }

        Ok(Self {
            route_lit: route.route_lit,
            method: route.method,
            path_params,
            query_params,
            state: route
                .state
                .unwrap_or_else(|| guess_state_type(&function.sig)),
            oapi_options: route.oapi_options,
            fn_name: function.sig.ident.clone(),
            debug: route.debug,
        })
    }

    fn path_param_struct_name(&self) -> Ident {
        let fn_name_pascal = self.fn_name.to_string().to_case(Case::Pascal);

        format_ident!("{}Path", fn_name_pascal, span = self.fn_name.span())
    }

    fn query_param_struct_name(&self) -> Ident {
        let fn_name_pascal = self.fn_name.to_string().to_case(Case::Pascal);

        format_ident!("{}Query", fn_name_pascal, span = self.fn_name.span())
    }

    pub fn path_extractor(&self) -> (Option<TokenStream2>, TokenStream2) {
        if !self
            .path_params
            .iter()
            .any(|(_, param, _)| param.captures())
        {
            return (None, quote! { ::axum::extract::Path<()> });
        }

        let path_iter = self
            .path_params
            .iter()
            .filter_map(|(_slash, path_param, _)| path_param.capture());
        let idents = path_iter.clone().map(|item| item.0);
        let name = self.path_param_struct_name();
        (
            Some(quote! {
                ::axum::extract::Path(#name {
                    #(#idents,)*
                }): ::axum::extract::Path<#name>,
            }),
            quote! { ::axum::extract::Path<#name> },
        )
    }

    pub fn query_extractor(&self) -> (Option<TokenStream2>, TokenStream2) {
        if self.query_params.is_empty() {
            return (None, quote! { ::axum::extract::Query<()> });
        }

        let idents = self.query_params.iter().map(|item| &item.0);
        let name = self.query_param_struct_name();
        (
            Some(quote! {
                ::axum::extract::Query(#name {
                    #(#idents,)*
                }): ::axum::extract::Query<#name>,
            }),
            quote! { ::axum::extract::Query<#name> },
        )
    }

    pub fn axum_debug_handler(&self) -> Option<TokenStream2> {
        if self.debug {
            Some(quote! {
                #[::axum::debug_handler]
            })
        } else {
            None
        }
    }

    pub fn debug_operation_input_output(&self, function: &ItemFn) -> TokenStream2 {
        if !self.debug {
            return quote! {};
        }

        let input_types = self
            .remaining_args(&function.sig.inputs)
            .filter_map(|arg| match arg {
                FnArg::Typed(pat_type) => Some(&pat_type.ty),
                FnArg::Receiver(_) => None,
            });

        let output_type = match &function.sig.output {
            ReturnType::Default => quote! { () },
            ReturnType::Type(_, ty) => quote! { #ty },
        };

        quote! {
            #[allow(dead_code)]
            const _: () = {
                const fn __debug_operation_input__<T: ::aide::OperationInput>() {}
                const fn __debug_operation_output__<T: ::aide::OperationOutput>() {}

                #(
                    __debug_operation_input__::<#input_types>();
                )*

                __debug_operation_output__::<#output_type>();
            };
        }
    }

    pub fn query_params_struct(&self, with_aide: bool) -> Option<TokenStream2> {
        match self.query_params.is_empty() {
            true => None,
            false => {
                let idents = self.query_params.iter().map(|item| &item.0);
                let types = self.query_params.iter().map(|item| &item.1);
                let docs = self.query_params.iter().map(|item| &item.2);
                let derive = match with_aide {
                    true => quote! { #[derive(::serde::Deserialize, ::schemars::JsonSchema)] },
                    false => quote! { #[derive(::serde::Deserialize)] },
                };
                let name = self.query_param_struct_name();
                Some(quote! {
                    #derive
                    struct #name {
                        #(#(#docs)* #idents: #types,)*
                    }
                })
            }
        }
    }

    pub fn path_params_struct(&self, with_aide: bool) -> Option<TokenStream2> {
        match self
            .path_params
            .iter()
            .any(|(_, param, _)| param.captures())
        {
            true => {
                let path_iter = self
                    .path_params
                    .iter()
                    .filter_map(|(_slash, path_param, docs)| {
                        path_param.capture().map(|p| (p, docs))
                    });
                let idents = path_iter.clone().map(|item| item.0.0);
                let types = path_iter.clone().map(|item| item.0.1);
                let docs = path_iter.clone().map(|item| item.1);
                let derive = match with_aide {
                    true => quote! { #[derive(::serde::Deserialize, ::schemars::JsonSchema)] },
                    false => quote! { #[derive(::serde::Deserialize)] },
                };
                let name = self.path_param_struct_name();
                Some(quote! {
                    #derive
                    struct #name {
                        #(#(#docs)* #idents: #types,)*
                    }
                })
            }
            false => None,
        }
    }

    pub fn extracted_idents(&self) -> Vec<Ident> {
        let mut idents = Vec::new();
        for (_slash, path_param, _) in &self.path_params {
            if let Some((ident, _ty)) = path_param.capture() {
                idents.push(ident.clone());
            }
        }
        for (ident, _ty, _) in &self.query_params {
            idents.push(ident.clone());
        }
        idents
    }

    /// Returns the function arguments that are not consumed by the route's
    /// path/query parameters.
    pub fn remaining_args<'a>(
        &self,
        args: impl IntoIterator<Item = &'a FnArg>,
    ) -> impl Iterator<Item = &'a FnArg> {
        args.into_iter().filter(|item| {
            let FnArg::Typed(pat_type) = item else {
                return true;
            };

            let Pat::Ident(pat_ident) = &*pat_type.pat else {
                return true;
            };

            !self.path_params.iter().any(|(_, path_param, _)| {
                path_param
                    .capture()
                    .is_some_and(|(path_ident, _)| path_ident == &pat_ident.ident)
            }) && !self
                .query_params
                .iter()
                .any(|(query_ident, _, _)| query_ident == &pat_ident.ident)
        })
    }

    /// The arguments not used in the route.
    /// Map the identifier to `___arg___{i}: Type`.
    pub fn remaining_pattypes_numbered(
        &self,
        args: &Punctuated<FnArg, Comma>,
    ) -> Punctuated<PatType, Comma> {
        self.remaining_args(args)
            .enumerate()
            .filter_map(|(i, item)| {
                if let FnArg::Typed(pat_type) = item {
                    let mut new_pat_type = pat_type.clone();
                    let ident = format_ident!("___arg___{}", i);
                    new_pat_type.pat = Box::new(parse_quote!(#ident));
                    Some(new_pat_type)
                } else {
                    unimplemented!("Self type is not supported")
                }
            })
            .collect()
    }

    pub fn ide_documentation_for_aide_methods(&self) -> TokenStream2 {
        let Some(options) = &self.oapi_options else {
            return quote! {};
        };
        let summary = options.summary.as_ref().map(|(ident, _)| {
            let method = Ident::new("summary", ident.span());
            quote!( let x = x.#method(""); )
        });
        let description = options.description.as_ref().map(|(ident, _)| {
            let method = Ident::new("description", ident.span());
            quote!( let x = x.#method(""); )
        });
        let id = options.id.as_ref().map(|(ident, _)| {
            let method = Ident::new("id", ident.span());
            quote!( let x = x.#method(""); )
        });
        let hidden = options.hidden.as_ref().map(|(ident, _)| {
            let method = Ident::new("hidden", ident.span());
            quote!( let x = x.#method(false); )
        });
        let tags = options.tags.as_ref().map(|(ident, _)| {
            let method = Ident::new("tag", ident.span());
            quote!( let x = x.#method(""); )
        });
        let security = options.security.as_ref().map(|(ident, _)| {
            let method = Ident::new("security_requirement_scopes", ident.span());
            quote!( let x = x.#method("", [""]); )
        });
        let responses = options.responses.as_ref().map(|(ident, _)| {
            let method = Ident::new("response", ident.span());
            quote!( let x = x.#method::<0, String>(); )
        });
        let transform = options.transform.as_ref().map(|(ident, _)| {
            let method = Ident::new("with", ident.span());
            quote!( let x = x.#method(|x|x); )
        });

        quote! {
            #[allow(unused)]
            #[allow(clippy::no_effect)]
            fn ____ide_documentation_for_aide____(x: ::aide::transform::TransformOperation) {
                #summary
                #description
                #id
                #hidden
                #tags
                #security
                #responses
                #transform
            }
        }
    }

    pub fn get_oapi_summary(&self) -> Option<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(summary) = &oapi_options.summary {
                return Some(summary.1.clone());
            }
        }
        None
    }

    pub fn get_oapi_description(&self) -> Option<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(description) = &oapi_options.description {
                return Some(description.1.clone());
            }
        }
        None
    }

    pub fn get_oapi_hidden(&self) -> Option<LitBool> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(hidden) = &oapi_options.hidden {
                return Some(hidden.1.clone());
            }
        }
        None
    }

    pub fn get_oapi_tags(&self) -> Vec<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(tags) = &oapi_options.tags {
                return tags.1.0.clone();
            }
        }
        Vec::new()
    }

    pub fn get_oapi_id(&self, sig: &Signature) -> Option<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(id) = &oapi_options.id {
                return Some(id.1.clone());
            }
        }
        Some(LitStr::new(&sig.ident.to_string(), sig.ident.span()))
    }

    pub fn get_oapi_transform(&self) -> syn::Result<Option<TokenStream2>> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(transform) = &oapi_options.transform {
                if transform.1.inputs.len() != 1 {
                    return Err(syn::Error::new(
                        transform.1.span(),
                        "expected a single identifier",
                    ));
                }

                let pat = transform.1.inputs.first().unwrap();
                let body = &transform.1.body;

                if let Pat::Ident(pat_ident) = pat {
                    let ident = &pat_ident.ident;
                    return Ok(Some(quote! {
                        let #ident = __op__;
                        let __op__ = #body;
                    }));
                } else {
                    return Err(syn::Error::new(
                        pat.span(),
                        "expected a single identifier without type",
                    ));
                }
            }
        }
        Ok(None)
    }

    pub fn get_oapi_responses(&self) -> Vec<(LitInt, Type)> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some((_ident, Responses(responses))) = &oapi_options.responses {
                return responses.clone();
            }
        }
        Default::default()
    }

    pub fn get_oapi_security(&self) -> Vec<(LitStr, Vec<LitStr>)> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some((_ident, Security(security))) = &oapi_options.security {
                return security
                    .iter()
                    .map(|(scheme, StrArray(scopes))| (scheme.clone(), scopes.clone()))
                    .collect();
            }
        }
        Default::default()
    }

    pub(crate) fn to_doc_comments(&self) -> TokenStream2 {
        let mut doc = format!(
            "# Handler information
- Method: `{}`
- Path: `{}`
- State: `{}`",
            self.method.to_axum_method_name(),
            self.route_lit.value(),
            self.state.to_token_stream(),
        );

        if let Some(options) = &self.oapi_options {
            let summary = options
                .summary
                .as_ref()
                .map(|(_, summary)| format!("\"{}\"", summary.value()))
                .unwrap_or("None".to_string());
            let description = options
                .description
                .as_ref()
                .map(|(_, description)| format!("\"{}\"", description.value()))
                .unwrap_or("None".to_string());
            let id = options
                .id
                .as_ref()
                .map(|(_, id)| format!("\"{}\"", id.value()))
                .unwrap_or("None".to_string());
            let hidden = options
                .hidden
                .as_ref()
                .map(|(_, hidden)| hidden.value().to_string())
                .unwrap_or("None".to_string());
            let tags = options
                .tags
                .as_ref()
                .map(|(_, tags)| tags.to_string())
                .unwrap_or("[]".to_string());
            let security = options
                .security
                .as_ref()
                .map(|(_, security)| security.to_string())
                .unwrap_or("{}".to_string());

            doc = format!(
                "{doc}

## OpenAPI
- Summary: `{summary}`
- Description: `{description}`
- Operation id: `{id}`
- Tags: `{tags}`
- Security: `{security}`
- Hidden: `{hidden}`
"
            );
        }

        quote!(
            #[doc = #doc]
        )
    }
}

fn guess_state_type(sig: &syn::Signature) -> Type {
    for arg in &sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            // Returns `T` if the type of the last segment is exactly `State<T>`.
            if let Type::Path(ty) = &*pat_type.ty {
                let last_segment = ty.path.segments.last().unwrap();
                if last_segment.ident == "State" {
                    if let PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if args.args.len() == 1 {
                            if let GenericArgument::Type(ty) = args.args.first().unwrap() {
                                return ty.clone();
                            }
                        }
                    }
                }
            }
        }
    }

    parse_quote! { () }
}
