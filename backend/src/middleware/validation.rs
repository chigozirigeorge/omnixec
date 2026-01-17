use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, extract::Request, middleware::Next,
    body::Body,
};
use serde::de::DeserializeOwned;
use validator::Validate;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Validation error: {0}")]
    InvalidInput(String),
}

impl IntoResponse for ValidationError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            ValidationError::InvalidInput(msg) => (StatusCode::BAD_REQUEST, msg),
        };

        let body = serde_json::json!({
            "error": error_message,
            "code": status.as_u16(),
        });

        (status, Json(body)).into_response()
    }
}

pub async fn validate_json<T: DeserializeOwned + Validate>(
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, ValidationError> {
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await.map_err(|_| {
        ValidationError::InvalidInput("Invalid request body".to_string())
    })?;

    // Try to deserialize and validate the JSON
    let value: T = serde_json::from_slice(&bytes)
        .map_err(|e| ValidationError::InvalidInput(format!("Invalid JSON: {}", e)))?;

    // Run validation
    value.validate().map_err(|e| {
        let errors = e
            .field_errors()
            .into_iter()
            .map(|(field, errors)| {
                let error_messages: Vec<String> = errors
                    .iter()
                    .map(|e| e.message.as_ref().map(|s| s.to_string()).unwrap_or_default())
                    .collect();
                format!("{}: {}", field, error_messages.join(", "))
            })
            .collect::<Vec<String>>()
            .join("; ");
            
        ValidationError::InvalidInput(format!("Validation failed: {}", errors))
    })?;

    // Rebuild the request with the validated body
    let body = Body::from(bytes);
    let req = Request::from_parts(parts, body);
    Ok(next.run(req).await)
}
