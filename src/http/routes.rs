use rocket::{Request, http::Status, response::status::Custom, serde::json::Json, tokio::task};
use serde::{Deserialize, Serialize};

use crate::core::{
    glossary::{GlossaryEntry, GlossaryOptions, create_micro_glossary_with_options},
    prompt::{
        PreparePromptRequest, PreparePromptResult, prepare_translation_prompt as prepare_prompt,
    },
};

const MAX_FUZZY_THRESHOLD: u32 = 4;

type ApiResult<T> = Result<Json<T>, Custom<Json<ApiErrorResponse>>>;

#[derive(Debug, Deserialize)]
pub(super) struct MatchGlossaryRequest {
    pub content: String,
    pub glossary: Vec<GlossaryEntry>,

    #[serde(default)]
    pub options: GlossaryOptions,
}

#[derive(Debug, Serialize)]
pub(super) struct MatchGlossaryResponse {
    pub micro_glossary: Vec<GlossaryEntry>,
}

#[derive(Debug, Serialize)]
pub(super) struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct ApiErrorResponse {
    pub error: ApiError,
}

#[derive(Debug, Serialize)]
pub(super) struct ApiError {
    pub code: String,
    pub message: String,
}

#[rocket::get("/health")]
pub(super) fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

#[rocket::post("/glossary/match", format = "json", data = "<request>")]
pub(super) async fn match_glossary(
    request: Json<MatchGlossaryRequest>,
) -> ApiResult<MatchGlossaryResponse> {
    let request = request.into_inner();

    validate_content(&request.content, "content")?;
    validate_options(request.options)?;

    let result = task::spawn_blocking(move || {
        create_micro_glossary_with_options(&request.content, &request.glossary, request.options)
    })
    .await
    .map_err(join_error)?
    .map_err(core_error)?;

    Ok(Json(MatchGlossaryResponse {
        micro_glossary: result,
    }))
}

#[rocket::post("/prompts/translation", format = "json", data = "<request>")]
pub(super) async fn prepare_translation_prompt(
    request: Json<PreparePromptRequest>,
) -> ApiResult<PreparePromptResult> {
    let request = request.into_inner();

    validate_content(&request.chapter, "chapter")?;
    validate_content(&request.translation_prompt, "translation_prompt")?;
    validate_options(request.glossary_options)?;

    let result = task::spawn_blocking(move || prepare_prompt(request))
        .await
        .map_err(join_error)?
        .map_err(core_error)?;

    Ok(Json(result))
}

#[rocket::catch(default)]
pub(super) fn default_error(status: Status, _request: &Request<'_>) -> Json<ApiErrorResponse> {
    Json(ApiErrorResponse {
        error: ApiError {
            code: status
                .reason()
                .unwrap_or("request_error")
                .to_lowercase()
                .replace(' ', "_"),

            message: status.reason().unwrap_or("Request failed").to_string(),
        },
    })
}

fn validate_content(
    value: &str,
    field: &'static str,
) -> Result<(), Custom<Json<ApiErrorResponse>>> {
    if value.trim().is_empty() {
        return Err(bad_request(
            "empty_field",
            format!("'{field}' must not be empty."),
        ));
    }

    Ok(())
}

fn validate_options(options: GlossaryOptions) -> Result<(), Custom<Json<ApiErrorResponse>>> {
    if options.fuzzy_threshold > MAX_FUZZY_THRESHOLD {
        return Err(bad_request(
            "invalid_fuzzy_threshold",
            format!("'fuzzy_threshold' must not exceed {MAX_FUZZY_THRESHOLD}."),
        ));
    }

    Ok(())
}

fn bad_request(
    code: impl Into<String>,
    message: impl Into<String>,
) -> Custom<Json<ApiErrorResponse>> {
    Custom(
        Status::BadRequest,
        Json(ApiErrorResponse {
            error: ApiError {
                code: code.into(),
                message: message.into(),
            },
        }),
    )
}

fn core_error(error: anyhow::Error) -> Custom<Json<ApiErrorResponse>> {
    eprintln!("Core processing error: {error:#}");

    internal_error()
}

fn join_error(error: task::JoinError) -> Custom<Json<ApiErrorResponse>> {
    eprintln!("Blocking worker failed: {error}");

    internal_error()
}

fn internal_error() -> Custom<Json<ApiErrorResponse>> {
    Custom(
        Status::InternalServerError,
        Json(ApiErrorResponse {
            error: ApiError {
                code: "internal_error".to_string(),
                message: "Failed to process request.".to_string(),
            },
        }),
    )
}
