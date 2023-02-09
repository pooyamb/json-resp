use axum::{extract::Path, http::StatusCode, response::IntoResponse, routing::get, Router};
use json_resp::{JsonError, JsonResponse};

#[derive(JsonError)]
enum AppErrors {
    #[json_error(request, status = StatusCode::NOT_FOUND, code = "not-found")]
    NotFound,

    #[json_error(request, status = StatusCode::CONFLICT, code = "received-odd-number", hint = "Try an even number")]
    OddNotAllowed(&'static str),

    #[json_error(internal)]
    InternalError,
}

async fn number(Path(number): Path<u64>) -> impl IntoResponse {
    if number % 2 == 0 {
        Ok(JsonResponse::with_content("Welcome to the club"))
    } else if number == 7 {
        Err(AppErrors::InternalError)
    } else {
        Err(AppErrors::OddNotAllowed(
            "We don't accept odd numbers around here",
        ))
    }
}

async fn not_found() -> impl IntoResponse {
    AppErrors::NotFound
}

async fn index() -> impl IntoResponse {
    JsonResponse::with_content("Hello")
}

#[tokio::main]
async fn main() {
    let router = Router::new()
        .route("/", get(index))
        .route("/:number", get(number))
        .fallback(not_found);

    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(router.into_make_service())
        .await
        .unwrap()
}
