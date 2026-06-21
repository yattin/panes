use std::{fmt, time::Duration};

use reqwest::{header::HeaderMap, StatusCode};
use serde_json::Value;
use thiserror::Error;

const DEFAULT_MAX_ATTEMPTS: u32 = 3;
const DEFAULT_BASE_DELAY: Duration = Duration::from_millis(250);
const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(4);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Recoverability {
    Retryable,
    Fatal,
}

impl Recoverability {
    fn as_str(self) -> &'static str {
        match self {
            Self::Retryable => "retryable",
            Self::Fatal => "fatal",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RetryPolicy {
    max_attempts: u32,
    base_delay: Duration,
    max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            base_delay: DEFAULT_BASE_DELAY,
            max_delay: DEFAULT_MAX_DELAY,
        }
    }
}

impl RetryPolicy {
    #[cfg(test)]
    pub(super) fn new(max_attempts: u32, base_delay: Duration, max_delay: Duration) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            base_delay,
            max_delay,
        }
    }

    pub(super) fn retry_delay(
        &self,
        error: &AnthropicRequestError,
        failed_attempts: u32,
    ) -> Option<Duration> {
        if failed_attempts >= self.max_attempts
            || error.recoverability() != Recoverability::Retryable
        {
            return None;
        }

        error
            .retry_after()
            .or_else(|| Some(self.exponential_delay(failed_attempts)))
    }

    fn exponential_delay(&self, failed_attempts: u32) -> Duration {
        let shift = failed_attempts.saturating_sub(1).min(16);
        let multiplier = 1u32 << shift;
        self.base_delay
            .saturating_mul(multiplier)
            .min(self.max_delay)
    }
}

#[derive(Debug, Error)]
pub(super) enum AnthropicRequestError {
    #[error("failed to send Anthropic Messages request: {source}")]
    Transport {
        #[from]
        source: reqwest::Error,
    },
    #[error("{0}")]
    Http(AnthropicHttpError),
}

impl AnthropicRequestError {
    pub(super) fn http(status: StatusCode, headers: &HeaderMap, body: String) -> Self {
        Self::Http(AnthropicHttpError::from_http_response(
            status,
            retry_after(headers),
            body,
        ))
    }

    fn recoverability(&self) -> Recoverability {
        match self {
            Self::Transport { source } => {
                if source.is_builder() || source.is_body() || source.is_decode() {
                    Recoverability::Fatal
                } else {
                    Recoverability::Retryable
                }
            }
            Self::Http(error) => error.recoverability(),
        }
    }

    fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::Transport { .. } => None,
            Self::Http(error) => error.retry_after,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct AnthropicHttpError {
    status: StatusCode,
    error_type: Option<String>,
    message: Option<String>,
    retry_after: Option<Duration>,
    body: String,
}

impl AnthropicHttpError {
    fn from_http_response(status: StatusCode, retry_after: Option<Duration>, body: String) -> Self {
        let (error_type, message) = parse_error_body(&body);
        Self {
            status,
            error_type,
            message,
            retry_after,
            body,
        }
    }

    fn recoverability(&self) -> Recoverability {
        if let Some(error_type) = self.error_type.as_deref() {
            return recoverability_for_error_type(error_type);
        }

        recoverability_for_status(self.status)
    }
}

impl fmt::Display for AnthropicHttpError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let recoverability = self.recoverability().as_str();
        let error_type = self.error_type.as_deref().unwrap_or("unknown_error");
        let message = self.message.as_deref().unwrap_or(self.body.as_str());
        write!(
            formatter,
            "Anthropic Messages request failed with {} ({error_type}, {recoverability}): {message}",
            self.status
        )
    }
}

pub(super) fn stream_error_message(value: &Value) -> String {
    let error = value.get("error").unwrap_or(value);
    let error_type = error
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown_error");
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("Anthropic stream error");
    let recoverability = recoverability_for_error_type(error_type).as_str();

    format!("Anthropic stream error ({error_type}, {recoverability}): {message}")
}

fn parse_error_body(body: &str) -> (Option<String>, Option<String>) {
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return (None, None);
    };
    let error = value.get("error").unwrap_or(&value);
    let error_type = error
        .get("type")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned);
    (error_type, message)
}

fn recoverability_for_status(status: StatusCode) -> Recoverability {
    if status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::REQUEST_TIMEOUT
        || status == StatusCode::CONFLICT
        || status.as_u16() == 529
        || status.is_server_error()
    {
        Recoverability::Retryable
    } else {
        Recoverability::Fatal
    }
}

fn recoverability_for_error_type(error_type: &str) -> Recoverability {
    match error_type {
        "rate_limit_error" | "overloaded_error" | "api_error" => Recoverability::Retryable,
        _ => Recoverability::Fatal,
    }
}

fn retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_rate_limit_and_overload_as_retryable() {
        let rate_limited = AnthropicHttpError::from_http_response(
            StatusCode::TOO_MANY_REQUESTS,
            None,
            r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#.to_string(),
        );
        let overloaded = AnthropicHttpError::from_http_response(
            StatusCode::from_u16(529).unwrap(),
            None,
            r#"{"error":{"type":"overloaded_error","message":"try again"}}"#.to_string(),
        );

        assert_eq!(rate_limited.recoverability(), Recoverability::Retryable);
        assert_eq!(overloaded.recoverability(), Recoverability::Retryable);
        assert!(rate_limited.to_string().contains("retryable"));
        assert!(overloaded.to_string().contains("retryable"));
    }

    #[test]
    fn classifies_auth_and_invalid_request_as_fatal() {
        let auth = AnthropicHttpError::from_http_response(
            StatusCode::UNAUTHORIZED,
            None,
            r#"{"error":{"type":"authentication_error","message":"bad key"}}"#.to_string(),
        );
        let invalid = AnthropicHttpError::from_http_response(
            StatusCode::BAD_REQUEST,
            None,
            r#"{"error":{"type":"invalid_request_error","message":"bad request"}}"#.to_string(),
        );

        assert_eq!(auth.recoverability(), Recoverability::Fatal);
        assert_eq!(invalid.recoverability(), Recoverability::Fatal);
    }

    #[test]
    fn retry_policy_uses_retry_after_before_exponential_backoff() {
        let policy = RetryPolicy::new(3, Duration::from_millis(100), Duration::from_secs(1));
        let error = AnthropicRequestError::Http(AnthropicHttpError::from_http_response(
            StatusCode::TOO_MANY_REQUESTS,
            Some(Duration::from_secs(2)),
            r#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#.to_string(),
        ));

        assert_eq!(policy.retry_delay(&error, 1), Some(Duration::from_secs(2)));
    }

    #[test]
    fn retry_policy_caps_exponential_backoff_and_attempts() {
        let policy = RetryPolicy::new(3, Duration::from_millis(300), Duration::from_millis(500));
        let error = AnthropicRequestError::Http(AnthropicHttpError::from_http_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            None,
            r#"{"error":{"type":"api_error","message":"later"}}"#.to_string(),
        ));

        assert_eq!(
            policy.retry_delay(&error, 1),
            Some(Duration::from_millis(300))
        );
        assert_eq!(
            policy.retry_delay(&error, 2),
            Some(Duration::from_millis(500))
        );
        assert_eq!(policy.retry_delay(&error, 3), None);
    }

    #[test]
    fn stream_error_message_includes_recoverability() {
        let message = stream_error_message(&serde_json::json!({
            "type": "error",
            "error": {
                "type": "overloaded_error",
                "message": "try again"
            }
        }));

        assert!(message.contains("overloaded_error"));
        assert!(message.contains("retryable"));
    }
}
