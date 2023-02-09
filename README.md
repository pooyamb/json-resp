# Json-resp

This crate provides a success and an error response for Apis, with utilities and macros to ease the generation
of responses and openapi docs(with `openapi` feature).(Only axum supported right now)

Check out the examples for full explanation.

## Success

The success response looks like:

```json5
{
    "status": 200,
    "content": C, // C should implement serde::Serialize
    "meta": M // M should implement serde::Serialize
}
```

And can be produces with builder pattern:

```rust
JsonResponse::with_content(content).meta(meta)
```

## Errors

The error response looks like:

```json5
{
    "status": 404,
    "code": "error code here",
    "hint": "do something", // Optional
    "content": C // C should implement serde::Serialize
}
```

And can be produces with a derive macro, openapi docs will be generated too.

```rust
#[derive(JsonError)]
enum MyAppErrors{
    #[json_error(request, status=404, code="does-not-exist", hint="some hint")]
    DoesNotExist,
    
    #[json_error(request, status=404, code="does-not-exist")]
    Validation(ValidationErrors),

    #[json_error(internal)]
    SomethingWentWrong
}
```

And just use it in your handlers:

```rust
async fn my_handler() -> Result<MyResponse, MyAppErrors>{
    Err(MyAppErrors::DoesNotExist)
}
```
