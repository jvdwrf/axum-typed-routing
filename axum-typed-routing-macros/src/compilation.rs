use quote::ToTokens;
use syn::{Attribute, Expr, Lit, LitBool, PatType};

use crate::parsing::OapiOptions;

use super::*;

pub struct CompiledRoute {
    pub method: Method,
    #[allow(clippy::type_complexity)]
    pub path_params: Vec<(Slash, Ident, Option<(Colon, Box<Type>)>)>,
    pub query_params: Vec<(Ident, Box<Type>)>,
    pub state: Type,
    pub route_lit: LitStr,
    pub oapi_options: Option<OapiOptions>,
}

impl CompiledRoute {
    pub fn to_axum_path_string(&self) -> String {
        let mut path = String::new();
        for (_slash, ident, colon) in &self.path_params {
            path.push('/');
            if colon.is_some() {
                path.push(':');
            }
            path.push_str(&ident.to_string());
        }
        path
    }

    /// Removes the arguments in `route` from `args`, and merges them in the output.
    pub fn from_route(route: Route, sig: &Signature, with_aide: bool) -> syn::Result<Self> {
        if !with_aide && route.oapi_options.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "Use `api_route` instead of `route` to use OpenAPI options",
            ));
        }

        let mut arg_map = sig
            .inputs
            .iter()
            .filter_map(|item| match item {
                syn::FnArg::Receiver(_) => None,
                syn::FnArg::Typed(pat_type) => Some(pat_type),
            })
            .filter_map(|pat_type| match &*pat_type.pat {
                syn::Pat::Ident(ident) => Some((ident.ident.clone(), pat_type.ty.clone())),
                _ => None,
            })
            .collect::<HashMap<_, _>>();

        let mut path_params = Vec::new();
        for (slash, colon, ident) in route.path_params {
            if let Some(colon) = colon {
                let (ident, ty) = arg_map.remove_entry(&ident).ok_or_else(|| {
                    syn::Error::new(
                        ident.span(),
                        format!("path parameter `{}` not found in function arguments", ident),
                    )
                })?;
                path_params.push((slash, ident, Some((colon, ty))))
            } else {
                path_params.push((slash, ident, None))
            }
        }

        let mut query_params = Vec::new();
        for ident in route.query_params {
            let (ident, ty) = arg_map.remove_entry(&ident).ok_or_else(|| {
                syn::Error::new(
                    ident.span(),
                    format!(
                        "query parameter `{}` not found in function arguments",
                        ident
                    ),
                )
            })?;
            query_params.push((ident, ty));
        }

        Ok(Self {
            route_lit: route.route_lit,
            method: route.method,
            path_params,
            query_params,
            state: route.state.unwrap_or_else(|| guess_state_type(sig)),
            oapi_options: route.oapi_options,
        })
    }

    pub fn path_extractor(&self) -> Option<TokenStream2> {
        match self.path_params.is_empty() {
            true => None,
            false => {
                let path_iter = self
                    .path_params
                    .iter()
                    .filter_map(|(_slash, ident, ty)| ty.as_ref().map(|(_colon, ty)| (ident, ty)));
                let idents = path_iter.clone().map(|item| item.0);
                let types = path_iter.clone().map(|item| item.1);
                Some(quote! {
                    ::axum::extract::Path((#(#idents,)*)): ::axum::extract::Path<(#(#types,)*)>,
                })
            }
        }
    }

    pub fn query_extractor(&self) -> Option<TokenStream2> {
        match self.query_params.is_empty() {
            true => None,
            false => {
                let idents = self.query_params.iter().map(|item| &item.0);
                let types = self.query_params.iter().map(|item| &item.1);
                Some(quote! {
                    #[allow(unused)]
                    ::axum::extract::Query((#( #idents,)*)): ::axum::extract::Query<(#(#types,)*)>,
                })
            }
        }
    }

    pub fn extracted_idents(&self) -> Vec<Ident> {
        let mut idents = Vec::new();
        for (_slash, ident, colon) in &self.path_params {
            if let Some((_colon, _ty)) = colon {
                idents.push(ident.clone());
            }
        }
        for (ident, _ty) in &self.query_params {
            idents.push(ident.clone());
        }
        idents
    }

    /// The arguments not used in the route.
    /// Map the identifier to `___arg___{i}: Type`.
    pub fn remaining_pattypes_numbered(
        &self,
        args: &Punctuated<FnArg, Comma>,
    ) -> Punctuated<PatType, Comma> {
        args.iter()
            .enumerate()
            .filter_map(|(i, item)| {
                if let FnArg::Typed(pat_type) = item {
                    if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                        if self.path_params.iter().any(|(_slash, path_ident, colon)| {
                            colon.is_some() && path_ident == &pat_ident.ident
                        }) || self
                            .query_params
                            .iter()
                            .any(|(query_ident, _)| query_ident == &pat_ident.ident)
                        {
                            return None;
                        }
                    }

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

    pub fn get_oapi_summary(&self, attrs: &[Attribute]) -> Option<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(summary) = &oapi_options.summary {
                return Some(summary.clone());
            }
        }
        doc_iter(attrs).next().map(|item| item.clone())
    }

    pub fn get_oapi_description(&self, attrs: &[Attribute]) -> Option<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(description) = &oapi_options.description {
                return Some(description.clone());
            }
        }
        doc_iter(attrs)
            .skip(2)
            .map(|item| item.value())
            .reduce(|mut acc, item| {
                acc.push('\n');
                acc.push_str(&item);
                acc
            })
            .map(|item| LitStr::new(&item, proc_macro2::Span::call_site()))
    }

    pub fn get_oapi_hidden(&self) -> Option<LitBool> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(hidden) = &oapi_options.hidden {
                return Some(hidden.clone());
            }
        }
        None
    }

    pub fn get_oapi_tags(&self) -> Vec<LitStr> {
        if let Some(oapi_options) = &self.oapi_options {
            if let Some(tags) = &oapi_options.tags {
                return tags.clone();
            }
        }
        Vec::new()
    }

    pub(crate) fn to_doc_comments(&self, sig: &Signature) -> TokenStream2 {
        let doc = format!(
            "## Handler information
- Path: `{} {}`
- Signature: 
    ```rust
    {}
    ```",
            self.method.to_axum_method_name(),
            self.route_lit.value(),
            sig.to_token_stream()
        );
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

fn doc_iter(attrs: &[Attribute]) -> impl Iterator<Item = &LitStr> + '_ {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .map(|attr| {
            let Meta::NameValue(meta) = &attr.meta else {
                panic!("doc attribute is not a name-value attribute");
            };
            let Expr::Lit(lit) = &meta.value else {
                panic!("doc attribute is not a string literal");
            };
            let Lit::Str(lit_str) = &lit.lit else {
                panic!("doc attribute is not a string literal");
            };
            lit_str
        })
}
