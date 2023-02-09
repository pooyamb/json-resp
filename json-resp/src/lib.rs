//! This crate contains a set of structs and macros to ease the implementation of REST apis

mod response;

pub use json_resp_derive::JsonError;
pub use response::{JsonError, JsonListMeta, JsonResponse, Nothing};

#[cfg(feature = "openapi")]
mod utoipa_impls;

#[cfg(feature = "openapi")]
pub use utoipa_impls::CombineErrors;

pub type JsonResult<T, E = Nothing> = Result<JsonResponse<T>, JsonError<E>>;

#[doc(hidden)]
pub mod __private {
    pub use axum::response::{IntoResponse, Response};

    #[cfg(feature = "log")]
    pub use log::error as log_error;

    #[cfg(feature = "openapi")]
    pub mod utoipa {
        pub use utoipa::{
            openapi::{
                ContentBuilder, KnownFormat, ObjectBuilder, Ref, RefOr, Response, ResponseBuilder,
                ResponsesBuilder, Schema, SchemaFormat, SchemaType,
            },
            IntoResponses, ToResponse, ToSchema,
        };
    }
}
