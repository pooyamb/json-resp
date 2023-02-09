use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Router};
use schemas::HelloResponse;
use serde::de::{value, Error};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use json_resp::{JsonError, JsonResponse, CombineErrors};

mod schemas {
    use serde::Serialize;
    use utoipa::ToSchema;

    #[derive(Serialize, ToSchema)]
    pub struct HelloResponse {
        pub number: u64,
        pub string: String,
    }
}

#[derive(JsonError)]
#[json_error(internal_code="500 internal")]
enum AppErrors {
    #[json_error(request, status=StatusCode::NOT_FOUND, code="404 not-found", description="The page does not exist")]
    NotFound,
    
    #[json_error(request, status=StatusCode::NOT_FOUND, code="4042 not-found")]
    NotFound2,

    #[json_error(internal)]
    InternalError,
    
    // Only one InternalError api doc will be generated, no matter how many of them we have
    // inner error will be logged using log crate(log::error)
    #[json_error(internal)]
    AnotherInternalError(value::Error),
}

#[derive(OpenApi)]
#[openapi(
    paths(index),
    components(
        schemas(schemas::HelloResponse, AppErrorsOai::NotFound, AppErrorsOai::NotFound2, AppErrorsOai::InternalError),
    )
)]
struct AppApi;

#[utoipa::path(
    get,
    path = "/{name}",
    responses(
        // JsonResponse has to be inlined otherwise it will cause naming conflicts
        (status=200, body=inline(JsonResponse<schemas::HelloResponse>)), 
        (status=201, body=inline(JsonResponse<schemas::HelloResponse, schemas::HelloResponse>)), 
        // CombineErrors can be used when 2 errors have the same status
        CombineErrors::<AppErrorsOai::NotFound, AppErrorsOai::NotFound2>,
        AppErrorsOai::InternalError
    )
)]
async fn index(Path(name): Path<String>) -> impl IntoResponse {
    match name.as_str() {
        "500" => Err(AppErrors::InternalError),
        "501" => Err(AppErrors::AnotherInternalError(value::Error::custom("Error"))),
        "404" => Err(AppErrors::NotFound),
        "4042" => Err(AppErrors::NotFound2),
        "meta" => Ok((
            StatusCode::CREATED,
            JsonResponse::with_content(HelloResponse {
                number: 1,
                string: name.to_string(),
            })
            .meta(HelloResponse {
                number: 2,
                string: name.to_string(),
            }),
        )
            .into_response()),
        _ => Ok(JsonResponse::with_content(HelloResponse {
            number: 1,
            string: name.to_string(),
        })
        .into_response()),
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    
    let openapi = AppApi::openapi();
    let openapi = SwaggerUi::new("/docs").url("/docs.json", openapi);

    let router = Router::new().route("/:name", get(index)).merge(openapi);

    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap()
}
