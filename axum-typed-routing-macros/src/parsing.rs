use syn::{spanned::Spanned, token::Brace, Expr, ExprClosure, FieldValue, Lit, LitBool, Member};

use super::*;

struct RouteLit {
    path_params: Vec<(Slash, Option<Colon>, Ident)>,
    query_params: Vec<Ident>,
}

impl Parse for RouteLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut path_params = Vec::new();
        while let Ok(slash) = input.parse::<Token![/]>() {
            let colon = input.parse::<Colon>().ok();
            let ident = input.parse::<Ident>()?;
            path_params.push((slash, colon, ident));
        }
        let mut query_params = Vec::new();
        if input.parse::<Token![?]>().is_ok() {
            while let Ok(ident) = input.parse::<Ident>() {
                query_params.push(ident);
                if input.parse::<Token![&]>().is_err() {
                    if !input.is_empty() {
                        Err(input.error("expected &"))?;
                    }
                    break;
                }
            }
        }
        Ok(RouteLit {
            path_params,
            query_params,
        })
    }
}

pub struct OapiOptions {
    pub summary: Option<LitStr>,
    pub description: Option<LitStr>,
    pub hidden: Option<LitBool>,
    pub tags: Option<Vec<LitStr>>,
    pub id: Option<LitStr>,
    pub transform: Option<ExprClosure>,
}

impl Parse for OapiOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut options = HashMap::new();
        while !input.is_empty() {
            let field_val = input.parse::<FieldValue>()?;
            let Member::Named(ident) = field_val.member else {
                return Err(input.error("expected named field"));
            };
            options.insert(ident.clone(), field_val.expr);
            input.parse::<Token![,]>().ok();
        }

        let this = Self {
            summary: match options.remove(&Ident::new("summary", Span::call_site())) {
                Some(option) => {
                    if let Expr::Lit(expr_lit) = option {
                        match expr_lit.lit {
                            Lit::Str(lit_str) => Some(lit_str),
                            _ => {
                                return Err(syn::Error::new(
                                    expr_lit.span(),
                                    "expected string literal",
                                ))
                            }
                        }
                    } else {
                        return Err(syn::Error::new(option.span(), "expected string literal"));
                    }
                }
                None => None,
            },
            description: match options.remove(&Ident::new("description", Span::call_site())) {
                Some(option) => {
                    if let Expr::Lit(expr_lit) = option {
                        match expr_lit.lit {
                            Lit::Str(lit_str) => Some(lit_str),
                            _ => {
                                return Err(syn::Error::new(
                                    expr_lit.span(),
                                    "expected string literal",
                                ))
                            }
                        }
                    } else {
                        return Err(syn::Error::new(option.span(), "expected string literal"));
                    }
                }
                None => None,
            },
            hidden: match options.remove(&Ident::new("hidden", Span::call_site())) {
                Some(option) => {
                    if let Expr::Lit(expr_lit) = option {
                        match expr_lit.lit {
                            Lit::Bool(lit_bool) => Some(lit_bool),
                            _ => {
                                return Err(syn::Error::new(
                                    expr_lit.span(),
                                    "expected boolean literal",
                                ))
                            }
                        }
                    } else {
                        return Err(syn::Error::new(option.span(), "expected boolean literal"));
                    }
                }
                None => None,
            },
            tags: match options.remove(&Ident::new("tags", Span::call_site())) {
                Some(option) => {
                    if let Expr::Array(expr_array) = option {
                        let mut tags = Vec::new();
                        for expr in expr_array.elems {
                            if let Expr::Lit(expr_lit) = expr {
                                match expr_lit.lit {
                                    Lit::Str(lit_str) => tags.push(lit_str),
                                    _ => {
                                        return Err(syn::Error::new(
                                            expr_lit.span(),
                                            "expected string literal",
                                        ))
                                    }
                                }
                            } else {
                                return Err(syn::Error::new(
                                    expr.span(),
                                    "expected string literal",
                                ));
                            }
                        }
                        Some(tags)
                    } else {
                        return Err(syn::Error::new(option.span(), "expected array literal"));
                    }
                }
                None => None,
            },
            id: match options.remove(&Ident::new("id", Span::call_site())) {
                Some(option) => {
                    if let Expr::Lit(expr_lit) = option {
                        match expr_lit.lit {
                            Lit::Str(lit_str) => Some(lit_str),
                            _ => {
                                return Err(syn::Error::new(
                                    expr_lit.span(),
                                    "expected string literal",
                                ))
                            }
                        }
                    } else {
                        return Err(syn::Error::new(option.span(), "expected string literal"));
                    }
                }
                None => None,
            },
            transform: match options.remove(&Ident::new("transform", Span::call_site())) {
                Some(option) => {
                    if let Expr::Closure(expr_closure) = option {
                        Some(expr_closure)
                    } else {
                        return Err(syn::Error::new(
                            option.span(),
                            "expected closure expression",
                        ));
                    }
                }
                None => None,
            },
        };

        if !options.is_empty() {
            return Err(syn::Error::new(
                options.keys().next().unwrap().span(),
                "unexpected field, expected one of (summary, description, hidden, tags, id, transform)",
            ));
        }

        Ok(this)
    }
}

pub struct Route {
    pub method: Method,
    pub path_params: Vec<(Slash, Option<Colon>, Ident)>,
    pub query_params: Vec<Ident>,
    pub state: Option<Type>,
    pub route_lit: LitStr,
    pub oapi_options: Option<OapiOptions>,
}

impl Parse for Route {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method = input.parse::<Method>()?;
        let route_lit = input.parse::<LitStr>()?;
        let route = route_lit.parse::<RouteLit>()?;
        let state = match input.parse::<kw::with>() {
            Ok(_) => Some(input.parse::<Type>()?),
            Err(_) => None,
        };
        let oapi_options = input
            .peek(Brace)
            .then(|| {
                let inner;
                braced!(inner in input);
                inner.parse::<OapiOptions>()
            })
            .transpose()?;

        Ok(Route {
            method,
            path_params: route.path_params,
            query_params: route.query_params,
            state,
            route_lit,
            oapi_options,
        })
    }
}

pub enum Method {
    Get(Span),
    Post(Span),
    Put(Span),
    Delete(Span),
    Head(Span),
    Connect(Span),
    Options(Span),
    Trace(Span),
}

impl Parse for Method {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;
        match ident.to_string().to_uppercase().as_str() {
            "GET" => Ok(Self::Get(ident.span())),
            "POST" => Ok(Self::Post(ident.span())),
            "PUT" => Ok(Self::Put(ident.span())),
            "DELETE" => Ok(Self::Delete(ident.span())),
            "HEAD" => Ok(Self::Head(ident.span())),
            "CONNECT" => Ok(Self::Connect(ident.span())),
            "OPTIONS" => Ok(Self::Options(ident.span())),
            "TRACE" => Ok(Self::Trace(ident.span())),
            _ => Err(input
                .error("expected one of (GET, POST, PUT, DELETE, HEAD, CONNECT, OPTIONS, TRACE)")),
        }
    }
}

impl Method {
    pub fn to_axum_method_name(&self) -> Ident {
        match self {
            Self::Get(span) => Ident::new("get", *span),
            Self::Post(span) => Ident::new("post", *span),
            Self::Put(span) => Ident::new("put", *span),
            Self::Delete(span) => Ident::new("delete", *span),
            Self::Head(span) => Ident::new("head", *span),
            Self::Connect(span) => Ident::new("connect", *span),
            Self::Options(span) => Ident::new("options", *span),
            Self::Trace(span) => Ident::new("trace", *span),
        }
    }
}

mod kw {
    syn::custom_keyword!(with);
}
