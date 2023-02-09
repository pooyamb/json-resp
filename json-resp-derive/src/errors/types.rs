use crate::ctxt::Ctxt;
use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Expr, ExprParen, ExprTuple, Lit, LitInt, LitStr, Path};
use syn::{Ident, Variant};

#[derive(Clone)]
pub(crate) enum JsonError {
    RequestError {
        naive: bool,
        variant: Ident,
        status: StatusExpr,
        code: LitStr,
        hint: Option<LitStr>,
        description: Option<LitStr>,
    },
    InternalError {
        naive: bool,
        variant: Ident,
    },
}

impl JsonError {
    fn variant(&self) -> &Ident {
        match self {
            Self::InternalError { variant, .. } => variant,
            Self::RequestError { variant, .. } => variant,
        }
    }

    fn status(&self) -> Option<&StatusExpr> {
        match self {
            Self::InternalError { .. } => None,
            Self::RequestError { status, .. } => Some(status),
        }
    }

    pub(crate) fn from_variant(variant: &Variant, ctxt: &Ctxt) -> Option<Self> {
        for attr in &variant.attrs {
            let ident = &attr.path.get_ident();
            if let Some(ident_string) = ident.map(|i| i.to_string()) {
                if ident_string != "json_error" {
                    continue;
                }

                return match Self::from_attr(
                    attr,
                    variant.fields.is_empty(),
                    variant.ident.clone(),
                    ctxt,
                ) {
                    Some(attrs) => Some(attrs),
                    None => return None,
                };
            }
        }
        ctxt.error_spanned_by(
            variant.ident.clone(),
            "All enum variants should have a api_erorr attribute",
        );
        None
    }

    fn from_attr(attr: &Attribute, naive: bool, variant: Ident, ctxt: &Ctxt) -> Option<Self> {
        let tokens = attr.tokens.to_owned();

        let (mode, others) = if let Some(attrs) = extract_mode(tokens) {
            attrs
        } else {
            ctxt.error_spanned_by(
                attr.path.clone(),
                "The first attribute is required and should be either `request` or `internal`",
            );
            return None;
        };

        if mode == ErrorType::Internal {
            return Some(JsonError::InternalError { naive, variant });
        }

        let mut status: Option<StatusExpr> = None;
        let mut code: Option<LitStr> = None;
        let mut hint: Option<LitStr> = None;
        let mut description: Option<LitStr> = None;

        let mut wrong_status_or_code = false;

        for attr in others {
            if let Expr::Assign(expr) = attr {
                let lhs = if let Some(ident_str) = extract_ident_str(&expr.left) {
                    ident_str
                } else {
                    ctxt.error_spanned_by(
                        expr.left,
                        "Assignments should be in form of `var = value`.",
                    );
                    return None;
                };

                match lhs.as_str() {
                    "status" => {
                        if let Some(val) = extract_status(&expr.right) {
                            status = Some(val);
                        } else {
                            wrong_status_or_code = true;
                            ctxt.error_spanned_by(
                                expr.right,
                                "status should be either a number \
                                 or a path(StatusCode::NOT_FOUND)",
                            );
                        }
                    }
                    "code" => {
                        if let Some(val) = extract_lit_str(&expr.right) {
                            code = Some(val);
                        } else {
                            wrong_status_or_code = true;
                            ctxt.error_spanned_by(expr.right, "code should be a str");
                        }
                    }
                    "hint" => {
                        if let Some(val) = extract_lit_str(&expr.right) {
                            hint = Some(val);
                        } else {
                            ctxt.error_spanned_by(expr.right, "hint should be a str");
                        }
                    }
                    "description" => {
                        if let Some(val) = extract_lit_str(&expr.right) {
                            description = Some(val);
                        } else {
                            ctxt.error_spanned_by(expr.right, "description should be a str");
                        }
                    }
                    _ => {
                        ctxt.error_spanned_by(expr.left, "Unknown attribute defined");
                    }
                }
            } else {
                ctxt.error_spanned_by(
                    attr,
                    "Only assignments are allowed to be used in error attributes.",
                );
            }
        }
        if status.is_some() && code.is_some() {
            Some(JsonError::RequestError {
                naive,
                variant,
                status: status.unwrap(),
                code: code.unwrap(),
                hint,
                description,
            })
        } else {
            if !wrong_status_or_code {
                ctxt.error_spanned_by(attr, "Both `status` and `code` should be defined.");
            }
            None
        }
    }
}

impl JsonError {
    #[allow(unused)]
    pub(crate) fn expand_match_condition(
        &self,
        type_ident: &Ident,
        internal_error_code: &str,
    ) -> TokenStream {
        match &self {
            Self::RequestError {
                naive,
                variant,
                status,
                code,
                hint,
                ..
            } => {
                let status = status.expand_statuscode();

                let hint = if let Some(hint) = hint {
                    quote!(Some(String::from(#hint)))
                } else {
                    quote!(None)
                };

                if *naive {
                    quote! {
                        #type_ident::#variant => ::json_resp::JsonError{
                            status: #status,
                            code: #code,
                            hint: #hint,
                            content: (),
                            ..::json_resp::JsonError::default()
                        }.into_response()
                    }
                } else {
                    quote! {
                        #type_ident::#variant(err) => ::json_resp::JsonError{
                            status: #status,
                            code: #code,
                            hint: #hint,
                            content: err,
                            ..::json_resp::JsonError::default()
                        }.into_response()
                    }
                }
            }
            Self::InternalError { naive, variant } => {
                let response = quote! {
                    ::json_resp::JsonError{
                        status: StatusCode::INTERNAL_SERVER_ERROR,
                        code: #internal_error_code.into(),
                        content: (),
                        ..::json_resp::JsonError::default()
                    }.into_response()
                };

                #[cfg(feature = "log")]
                let log_error = if *naive {
                    quote! {
                        /// Log the error
                        ::json_resp::__private::log_error!(
                            "{}::{}",
                            stringify!(#type_ident),
                            stringify!(#variant)
                        );
                    }
                } else {
                    quote! {
                        /// Log the error
                        ::json_resp::__private::log_error!(
                            "{}::{} {}",
                            stringify!(#type_ident),
                            stringify!(#variant),
                            err
                        );
                    }
                };
                #[cfg(not(feature = "log"))]
                let log_error: Option<TokenStream> = None;

                if *naive {
                    quote! {
                        #type_ident::#variant => {
                            #log_error
                            #response
                        }
                    }
                } else {
                    quote! {
                        #type_ident::#variant(err) => {
                            #log_error
                            #response
                        }
                    }
                }
            }
        }
    }

    fn expand_utoipa_schema_method(&self, name: &Ident) -> Option<TokenStream> {
        match self {
            Self::RequestError {
                naive,
                status,
                code,
                hint,
                ..
            } => {
                let status = status.expand_numeric();

                let content_expand = if *naive {
                    None
                } else {
                    Some(quote! {
                        .property(
                            "content",
                            ::json_resp::__private::utoipa::ObjectBuilder::new(),
                        )
                        .required("content")
                    })
                };

                let hint_expand = if let Some(hint) = hint {
                    quote! {
                        .property(
                            "hint",
                            ::json_resp::__private::utoipa::ObjectBuilder::new()
                                .schema_type(::json_resp::__private::utoipa::SchemaType::Integer)
                                .enum_values(Some([#hint]))
                                .example(Some(#hint.into())),
                        )
                        .required("hint")
                    }
                } else {
                    quote!()
                };

                Some(quote! {(
                    stringify!(#name),
                    ::json_resp::__private::utoipa::ObjectBuilder::new()
                        .property(
                            "status",
                            ::json_resp::__private::utoipa::ObjectBuilder::new()
                                .schema_type(::json_resp::__private::utoipa::SchemaType::Integer)
                                .enum_values(Some([#status]))
                                .example(Some(#status.into())))
                        .required("status")
                        .property(
                            "code",
                            ::json_resp::__private::utoipa::ObjectBuilder::new()
                                .schema_type(::json_resp::__private::utoipa::SchemaType::String)
                                .enum_values(Some([#code]))
                                .example(Some(#code.into())),
                        )
                        .required("code")
                        #hint_expand
                        #content_expand
                        .build()
                        .into(),
                )})
            }
            _ => None,
        }
    }

    fn expand_utoipa_response_method(&self, name: &Ident) -> Option<TokenStream> {
        match self {
            Self::RequestError {
                description,
                hint,
                code,
                ..
            } => {
                let description = description
                    .clone()
                    .ok_or_else(|| hint.clone())
                    .unwrap_or_else(|_| code.clone());
                Some(quote! {(
                    stringify!(#name),
                    ::json_resp::__private::utoipa::ResponseBuilder::new()
                        .description(#description)
                        .content(
                            "application/json",
                            ::json_resp::__private::utoipa::ContentBuilder::new()
                                .schema(
                                    ::json_resp::__private::utoipa::Ref::from_schema_name(
                                        <Self as ::json_resp::__private::utoipa::ToSchema>::schema().0,
                                    )
                                )
                                .build()
                                .into(),
                        )
                        .build()
                        .into(),
                )})
            }
            _ => None,
        }
    }

    fn expand_utoipa_intoresponse_method(&self) -> Option<TokenStream> {
        let status = self.status()?.expand_numeric();

        Some(quote! {
            ::json_resp::__private::utoipa::ResponsesBuilder::new()
                .response(
                    #status.to_string(),
                    <Self as ::json_resp::__private::utoipa::ToResponse>::response().1,
                )
                .build()
                .into()
        })
    }

    pub(crate) fn expand_utoipa_response(&self) -> Option<TokenStream> {
        let name = self.variant();
        let schema = self.expand_utoipa_schema_method(&name)?;
        let response = self.expand_utoipa_response_method(&name)?;
        let intoresponse = self.expand_utoipa_intoresponse_method()?;
        Some(quote!(
            pub struct #name;
            impl ::json_resp::__private::utoipa::ToSchema<'static> for #name {
                fn schema() -> (&'static str, ::json_resp::__private::utoipa::RefOr<::json_resp::__private::utoipa::Schema>) {
                    #schema
                }
            }

            impl ::json_resp::__private::utoipa::ToResponse<'static> for #name {
                fn response() -> (&'static str, ::json_resp::__private::utoipa::RefOr<::json_resp::__private::utoipa::Response>) {
                    #response
                }
            }

            impl ::json_resp::__private::utoipa::IntoResponses for #name {
                fn responses() -> std::collections::BTreeMap<
                    String,
                    ::json_resp::__private::utoipa::RefOr<::json_resp::__private::utoipa::Response>,
                > {
                    #intoresponse
                }
            }
        ))
    }

    pub(crate) fn expand_utoipa_internal_error(internal_error_code: &str) -> TokenStream {
        quote!(
            pub struct InternalError;


            impl ::json_resp::__private::utoipa::ToSchema<'static> for InternalError {
                fn schema() -> (&'static str, ::json_resp::__private::utoipa::RefOr<::json_resp::__private::utoipa::Schema>) {
                    (
                        "InternalError",
                        ::json_resp::__private::utoipa::ObjectBuilder::new()
                            .property(
                                "status",
                                ::json_resp::__private::utoipa::ObjectBuilder::new()
                                    .schema_type(::json_resp::__private::utoipa::SchemaType::Integer)
                                    .enum_values(Some(["500"]))
                                    .example(Some("500".into())),
                            )
                            .required("status")
                            .property(
                                "code",
                                ::json_resp::__private::utoipa::ObjectBuilder::new()
                                    .schema_type(::json_resp::__private::utoipa::SchemaType::String)
                                    .enum_values(Some([#internal_error_code]))
                                    .example(Some(#internal_error_code.into())),
                            )
                            .required("code")
                            .build()
                            .into(),
                    )
                }
            }

            impl ::json_resp::__private::utoipa::ToResponse<'static> for InternalError {
                fn response() -> (&'static str, ::json_resp::__private::utoipa::RefOr<::json_resp::__private::utoipa::Response>) {
                    (
                        "InternalError",
                        ::json_resp::__private::utoipa::ResponseBuilder::new()
                            .description("InternalError")
                            .content(
                                "application/json",
                                ::json_resp::__private::utoipa::ContentBuilder::new()
                                    .schema(
                                        ::json_resp::__private::utoipa::Ref::from_schema_name(
                                            <Self as ::json_resp::__private::utoipa::ToSchema>::schema().0,
                                        )
                                    )
                                    .build()
                                    .into(),
                            )
                            .build()
                            .into()
                        )
                }
            }

            impl ::json_resp::__private::utoipa::IntoResponses for InternalError {
                fn responses() -> std::collections::BTreeMap<
                    String,
                    ::json_resp::__private::utoipa::RefOr<::json_resp::__private::utoipa::Response>,
                > {
                    ::json_resp::__private::utoipa::ResponsesBuilder::new()
                        .response(
                            "500",
                            <Self as ::json_resp::__private::utoipa::ToResponse>::response().1,
                        )
                        .build()
                        .into()
                }
            }
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ErrorType {
    Request,
    Internal,
}

#[derive(Clone)]
pub(crate) enum StatusExpr {
    Lit(LitInt),
    Path(Path),
}

impl StatusExpr {
    pub fn expand_statuscode(&self) -> TokenStream {
        match self {
            Self::Path(p) => {
                quote! (#p)
            }
            Self::Lit(n) => {
                quote!(StatusCode::from_u16(#n).unwrap())
            }
        }
    }

    pub fn expand_numeric(&self) -> TokenStream {
        match self {
            Self::Path(p) => {
                quote!(StatusCode::as_u16(&#p))
            }
            Self::Lit(n) => {
                quote! (#n)
            }
        }
    }
}

fn extract_lit_str(expr: &Expr) -> Option<LitStr> {
    match expr {
        Expr::Lit(lit) => {
            if let Lit::Str(lit) = &lit.lit {
                return Some(lit.clone());
            }
        }
        _ => {}
    }
    None
}

fn extract_status(expr: &Expr) -> Option<StatusExpr> {
    match expr {
        Expr::Lit(lit) => {
            if let Lit::Int(lit) = &lit.lit {
                return Some(StatusExpr::Lit(lit.clone()));
            }
        }
        Expr::Path(p) => return Some(StatusExpr::Path(p.path.clone())),
        _ => {}
    }
    None
}

fn extract_ident_str(expr: &Expr) -> Option<String> {
    if let Expr::Path(path) = expr {
        Some(path.path.get_ident()?.to_string())
    } else {
        None
    }
}

fn extract_type(attr: &Expr) -> Option<ErrorType> {
    if let Expr::Path(path) = attr {
        match path.path.get_ident()?.to_string().as_str() {
            "request" => Some(ErrorType::Request),
            "internal" => Some(ErrorType::Internal),
            _ => return None,
        }
    } else {
        return None;
    }
}

fn extract_mode(tokens: TokenStream) -> Option<(ErrorType, Vec<Expr>)> {
    // Try to parse as a tuple
    if let Ok(tuple) = syn::parse2::<ExprTuple>(tokens.clone()) {
        let mut elems = tuple.elems.into_iter();
        return Some((extract_type(&elems.next()?)?, elems.collect()));
    }

    // Try to parse as (mode)
    if let Ok(mode) = syn::parse2::<ExprParen>(tokens) {
        return Some((extract_type(&mode.expr)?, Vec::new()));
    }

    None
}
