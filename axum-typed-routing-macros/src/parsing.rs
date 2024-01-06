use quote::ToTokens;
use syn::{
    parse2, spanned::Spanned, token::Brace, Expr, ExprArray, ExprClosure, FieldValue, Lit, LitBool,
    LitInt, Member,
};

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
    pub summary: Option<(Ident, LitStr)>,
    pub description: Option<(Ident, LitStr)>,
    pub id: Option<(Ident, LitStr)>,
    pub hidden: Option<(Ident, LitBool)>,
    pub tags: Option<(Ident, StrArray)>,
    pub security: Option<(Ident, Security)>,
    pub responses: Option<(Ident, Responses)>,
    pub transform: Option<(Ident, ExprClosure)>,
}

pub struct Security(pub Vec<(LitStr, StrArray)>);
impl Parse for Security {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner;
        braced!(inner in input);

        let mut arr = Vec::new();
        while !inner.is_empty() {
            let scheme = inner.parse::<LitStr>()?;
            let _ = inner.parse::<Token![:]>()?;
            let scopes = inner.parse::<StrArray>()?;
            let _ = inner.parse::<Token![,]>().ok();
            arr.push((scheme, scopes));
        }

        Ok(Self(arr))
    }
}

pub struct Responses(pub Vec<(LitInt, Type)>);
impl Parse for Responses {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner;
        braced!(inner in input);

        let mut arr = Vec::new();
        while !inner.is_empty() {
            let status = inner.parse::<LitInt>()?;
            let _ = inner.parse::<Token![:]>()?;
            let ty = inner.parse::<Type>()?;
            let _ = inner.parse::<Token![,]>().ok();
            arr.push((status, ty));
        }

        Ok(Self(arr))
    }
}

#[derive(Clone)]
pub struct StrArray(pub Vec<LitStr>);
impl Parse for StrArray {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let inner;
        bracketed!(inner in input);
        let mut arr = Vec::new();
        while !inner.is_empty() {
            arr.push(inner.parse::<LitStr>()?);
            inner.parse::<Token![,]>().ok();
        }
        Ok(Self(arr))
    }
}

impl Parse for OapiOptions {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut this = Self {
            summary: None,
            description: None,
            id: None,
            hidden: None,
            tags: None,
            security: None,
            responses: None,
            transform: None,
        };

        while !input.is_empty() {
            let ident = input.parse::<Ident>()?;
            let _ = input.parse::<Token![:]>()?;
            match ident.to_string().as_str() {
                "summary" => this.summary = Some((ident, input.parse()?)),
                "description" => this.description = Some((ident, input.parse()?)),
                "id" => this.id = Some((ident, input.parse()?)),
                "hidden" => this.hidden = Some((ident, input.parse()?)),
                "tags" => this.tags = Some((ident, input.parse()?)),
                "security" => this.security = Some((ident, input.parse()?)),
                "responses" => this.responses = Some((ident, input.parse()?)),
                "transform" => this.transform = Some((ident, input.parse()?)),
                _ => {
                    return Err(syn::Error::new(
                        ident.span(),
                        "unexpected field, expected one of (summary, description, id, hidden, tags, security, responses, transform)",
                    ))
                }
            }
            let _ = input.parse::<Token![,]>().ok();
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
