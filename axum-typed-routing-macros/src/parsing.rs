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

pub struct Route {
    pub method: Method,
    pub path_params: Vec<(Slash, Option<Colon>, Ident)>,
    pub query_params: Vec<Ident>,
    pub state: Option<Type>,
    pub route_lit: LitStr,
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

        Ok(Route {
            method,
            path_params: route.path_params,
            query_params: route.query_params,
            state,
            route_lit,
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
