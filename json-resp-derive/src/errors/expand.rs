use super::types::JsonError;
use crate::ctxt::Ctxt;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    punctuated::Punctuated, token::Comma, Attribute, Data, DataEnum, Expr, ExprAssign, ExprParen,
    Lit, Variant,
};

pub fn expand_derive(input: &syn::DeriveInput) -> Result<TokenStream, Vec<syn::Error>> {
    let ctxt = Ctxt::new();

    let config = Config::from_attrs(&input.attrs);

    let qoute = match &input.data {
        Data::Enum(DataEnum { variants, .. }) => expand_derive_enum(input, config, variants, &ctxt),
        _ => {
            ctxt.error_spanned_by(input, "Expected `enum`");
            None
        }
    };
    let qoute = match qoute {
        Some(cont) => cont,
        None => return Err(ctxt.check().unwrap_err()),
    };
    ctxt.check()?;
    Ok(qoute)
}

fn expand_derive_enum(
    input: &syn::DeriveInput,
    config: Config,
    variants: &Punctuated<Variant, Comma>,
    ctxt: &Ctxt,
) -> Option<TokenStream> {
    let name = &input.ident;
    let json_errors = match JsonErrors::from_variants(name.clone(), config, variants, ctxt) {
        Some(json_errors) => json_errors,
        None => return None,
    };
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let gen = quote! {
        impl #impl_generics ::json_resp::__private::IntoResponse for #name #ty_generics #where_clause {
            fn into_response(self) -> ::json_resp::__private::Response {
                match self{
                    #json_errors
                }
            }
        }
    };

    let docs_name = Ident::new(&format!("{}Oai", name), Span::call_site());

    #[cfg(feature = "openapi")]
    let gen = {
        let utoipa_inner = json_errors.into_utoipa_expand();

        // Utoipa impls
        quote! {
            #gen
            pub(crate) mod #docs_name{
                use super::*;

                #utoipa_inner
            }
        }
    };

    Some(gen)
}

pub struct Config {
    pub internal_error_code: String,
}

impl Config {
    fn from_attrs(attrs: &Vec<Attribute>) -> Self {
        let attrs = attrs
            .iter()
            .filter(|attr| {
                attr.path.get_ident().is_some()
                    && attr.path.get_ident().unwrap().to_string().as_str() == "json_error"
            })
            .next()
            .and_then(|attr| syn::parse2::<ExprParen>(attr.tokens.clone()).ok())
            .and_then(|attr| syn::parse2::<ExprAssign>(attr.expr.to_token_stream()).ok())
            .and_then(Self::extract_internal_code);

        match attrs {
            Some(internal_code) => Self {
                internal_error_code: internal_code,
            },
            None => {
                return Self {
                    internal_error_code: String::from("internal-error"),
                };
            }
        }
    }

    fn extract_internal_code(assign: ExprAssign) -> Option<String> {
        match *assign.left {
            Expr::Path(key)
                if key.path.get_ident().map(|k| k.to_string())
                    == Some("internal_code".to_string()) => {}
            _ => return None,
        };

        match *assign.right {
            Expr::Lit(lit) => match lit.lit {
                Lit::Str(lit) => Some(lit.value()),
                _ => None,
            },
            _ => None,
        }
    }
}

pub struct JsonErrors {
    ident: Ident,
    internal_err_code: String,
    errors: Vec<JsonError>,
}

impl JsonErrors {
    pub(crate) fn from_variants(
        ident: Ident,
        config: Config,
        variants: &Punctuated<Variant, Comma>,
        ctxt: &Ctxt,
    ) -> Option<Self> {
        let mut ret = Vec::new();
        for variant in variants.iter() {
            match JsonError::from_variant(variant, ctxt) {
                Some(err) => ret.push(err),
                None => {}
            }
        }
        Some(Self {
            ident,
            internal_err_code: config.internal_error_code,
            errors: ret,
        })
    }

    #[cfg(feature = "openapi")]
    pub(crate) fn into_utoipa_expand(self) -> JsonErrorUtoipaTypes {
        JsonErrorUtoipaTypes {
            internal_err_code: self.internal_err_code,
            errors: self.errors,
        }
    }
}

impl ToTokens for JsonErrors {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for err_type in &self.errors {
            let cond = err_type.expand_match_condition(&self.ident, &self.internal_err_code);
            tokens.append_all(quote!(#cond,));
        }
    }
}

pub struct JsonErrorUtoipaTypes {
    internal_err_code: String,
    errors: Vec<JsonError>,
}

impl ToTokens for JsonErrorUtoipaTypes {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let mut has_internal_error = false;

        for err_type in &self.errors {
            if let Some(gen) = err_type.expand_utoipa_response() {
                tokens.append_all(gen);
            } else {
                has_internal_error = true;
            }
        }

        if has_internal_error {
            tokens.append_all(JsonError::expand_utoipa_internal_error(
                &self.internal_err_code,
            ));
        }
    }
}
