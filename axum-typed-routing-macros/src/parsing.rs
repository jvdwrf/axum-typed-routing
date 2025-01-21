use core::panic;

use quote::ToTokens;
use syn::{
    token::{Brace, Star},
    Attribute, Expr, ExprClosure, Lit, LitBool, LitInt,
};

use super::*;

struct RouteParser {
    path_params: Vec<(Slash, PathParam)>,
    query_params: Vec<Ident>,
}

impl RouteParser {
    fn new(lit: LitStr) -> syn::Result<Self> {
        let val = lit.value();
        let span = lit.span();
        let split_route = val.split('?').collect::<Vec<_>>();
        if split_route.len() > 2 {
            return Err(syn::Error::new(span, "expected at most one '?'"));
        }

        let path = split_route[0];
        if !path.starts_with('/') {
            return Err(syn::Error::new(span, "expected path to start with '/'"));
        }
        let path = path.strip_prefix('/').unwrap();

        let mut path_params = Vec::new();
        #[allow(clippy::never_loop)]
        for path_param in path.split('/') {
            path_params.push((
                Slash(span),
                PathParam::new(path_param, span, Box::new(parse_quote!(()))),
            ));
        }

        let path_param_len = path_params.len();
        for (i, (_slash, path_param)) in path_params.iter().enumerate() {
            match path_param {
                PathParam::WildCard(_, _, _, _, _, _) => {
                    if i != path_param_len - 1 {
                        return Err(syn::Error::new(
                            span,
                            "wildcard path param must be the last path param",
                        ));
                    }
                }
                PathParam::Capture(_, _, _, _, _) => (),
                PathParam::Static(lit) => {
                    if lit.value() == "*" && i != path_param_len - 1 {
                        return Err(syn::Error::new(
                            span,
                            "wildcard path param must be the last path param",
                        ));
                    }
                }
            }
        }

        let mut query_params = Vec::new();
        if split_route.len() == 2 {
            let query = split_route[1];
            for query_param in query.split('&') {
                query_params.push(Ident::new(query_param, span));
            }
        }

        Ok(Self {
            path_params,
            query_params,
        })
    }
}

pub enum PathParam {
    WildCard(LitStr, Brace, Star, Ident, Box<Type>, Brace),
    Capture(LitStr, Brace, Ident, Box<Type>, Brace),
    Static(LitStr),
}

impl PathParam {
    pub fn captures(&self) -> bool {
        matches!(self, Self::Capture(..) | Self::WildCard(..))
    }

    // pub fn lit(&self) -> &LitStr {
    //     match self {
    //         Self::Capture(lit, _, _, _) => lit,
    //         Self::WildCard(lit, _, _, _) => lit,
    //         Self::Static(lit) => lit,
    //     }
    // }

    pub fn capture(&self) -> Option<(&Ident, &Type)> {
        match self {
            Self::Capture(_, _, ident, ty, _) => Some((ident, ty)),
            Self::WildCard(_, _, _, ident, ty, _) => Some((ident, ty)),
            _ => None,
        }
    }

    fn new(str: &str, span: Span, ty: Box<Type>) -> Self {
        if str.starts_with(':') {
            let str = str.strip_prefix(':').unwrap();
            Self::Capture(
                LitStr::new(str, span),
                Brace(span),
                Ident::new(str, span),
                ty,
                Brace(span),
            )
        } else if str.starts_with('*') && str.len() > 1 {
            let str = str.strip_prefix('*').unwrap();
            Self::WildCard(
                LitStr::new(str, span),
                Brace(span),
                Star(span),
                Ident::new(str, span),
                ty,
                Brace(span),
            )
        } else {
            Self::Static(LitStr::new(str, span))
        }
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

impl ToString for Security {
    fn to_string(&self) -> String {
        let mut s = String::new();
        s.push('{');
        for (i, (scheme, scopes)) in self.0.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&scheme.value());
            s.push_str(": ");
            s.push_str(&scopes.to_string());
        }
        s.push('}');
        s
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

impl ToString for Responses {
    fn to_string(&self) -> String {
        let mut s = String::new();
        s.push('{');
        for (i, (status, ty)) in self.0.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push_str(&status.to_string());
            s.push_str(": ");
            s.push_str(&ty.to_token_stream().to_string());
        }
        s.push('}');
        s
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

impl ToString for StrArray {
    fn to_string(&self) -> String {
        let mut s = String::new();
        s.push('[');
        for (i, lit) in self.0.iter().enumerate() {
            if i > 0 {
                s.push_str(", ");
            }
            s.push('"');
            s.push_str(&lit.value());
            s.push('"');
        }
        s.push(']');
        s
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

impl OapiOptions {
    pub fn merge_with_fn(&mut self, function: &ItemFn) {
        if self.description.is_none() {
            self.description = doc_iter(&function.attrs)
                .skip(2)
                .map(|item| item.value())
                .reduce(|mut acc, item| {
                    acc.push('\n');
                    acc.push_str(&item);
                    acc
                })
                .map(|item| (parse_quote!(description), parse_quote!(#item)))
        }
        if self.summary.is_none() {
            self.summary = doc_iter(&function.attrs)
                .next()
                .map(|item| (parse_quote!(summary), item.clone()))
        }
        if self.id.is_none() {
            let id = &function.sig.ident;
            self.id = Some((parse_quote!(id), LitStr::new(&id.to_string(), id.span())));
        }
    }
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

pub struct Route {
    pub method: Method,
    pub path_params: Vec<(Slash, PathParam)>,
    pub query_params: Vec<Ident>,
    pub state: Option<Type>,
    pub route_lit: LitStr,
    pub oapi_options: Option<OapiOptions>,
}

impl Parse for Route {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let method = input.parse::<Method>()?;
        let route_lit = input.parse::<LitStr>()?;
        let route_parser = RouteParser::new(route_lit.clone())?;
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
            path_params: route_parser.path_params,
            query_params: route_parser.query_params,
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
