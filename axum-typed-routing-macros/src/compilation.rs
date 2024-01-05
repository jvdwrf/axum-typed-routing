use quote::ToTokens;

use super::*;

#[derive(Debug)]
pub struct CompiledRoute {
    pub method: Method,
    #[allow(clippy::type_complexity)]
    pub path_params: Vec<(Slash, Ident, Option<(Colon, Box<Type>)>)>,
    pub query_params: Vec<(Ident, Box<Type>)>,
    pub state: Type,
    pub route_lit: LitStr,
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
    pub fn from_route(route: Route, sig: &Signature) -> syn::Result<Self> {
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

    // The arguments not used in the route.
    pub fn remaining_args(&self, args: &Punctuated<FnArg, Comma>) -> Punctuated<FnArg, Comma> {
        args.iter()
            .filter(|item| {
                if let FnArg::Typed(pat_type) = item {
                    if let syn::Pat::Ident(ident) = &*pat_type.pat {
                        if self
                            .path_params
                            .iter()
                            .any(|(_slash, path_ident, _)| path_ident == &ident.ident)
                            || self
                                .query_params
                                .iter()
                                .any(|(query_ident, _)| query_ident == &ident.ident)
                        {
                            return false;
                        }
                    }
                }
                true
            })
            .cloned()
            .collect()
    }

    pub(crate) fn to_doc_comments(&self, sig: &Signature) -> TokenStream2 {
        let doc = format!(
            "## Handler information
- Path: `{}`
- Signature: 
    ```rust
    {}
    ```",
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
        match arg {
            FnArg::Typed(pat_type) => {
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
            FnArg::Receiver(_) => {}
        }
    }

    parse_quote! { () }
}
