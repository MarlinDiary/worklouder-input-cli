use anyhow::Error;
use serde::Serialize;

pub const SUCCESS: i32 = 0;
pub const UNEXPECTED: i32 = 1;
pub const USAGE: i32 = 2;
pub const PROVIDER_UNAVAILABLE: i32 = 3;
pub const INVALID_DATA: i32 = 4;
pub const CONFLICT: i32 = 5;
pub const OPERATION_ROLLED_BACK: i32 = 6;
pub const ROLLBACK_FAILED: i32 = 7;
pub const PERMISSION_DENIED: i32 = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    Unexpected,
    ProviderUnavailable,
    InvalidData,
    Conflict,
    OperationRolledBack,
    RollbackFailed,
    PermissionDenied,
}

impl ErrorCode {
    pub fn exit_status(self) -> i32 {
        match self {
            Self::Unexpected => UNEXPECTED,
            Self::ProviderUnavailable => PROVIDER_UNAVAILABLE,
            Self::InvalidData => INVALID_DATA,
            Self::Conflict => CONFLICT,
            Self::OperationRolledBack => OPERATION_ROLLED_BACK,
            Self::RollbackFailed => ROLLBACK_FAILED,
            Self::PermissionDenied => PERMISSION_DENIED,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unexpected => "unexpected",
            Self::ProviderUnavailable => "provider-unavailable",
            Self::InvalidData => "invalid-data",
            Self::Conflict => "conflict",
            Self::OperationRolledBack => "operation-rolled-back",
            Self::RollbackFailed => "rollback-failed",
            Self::PermissionDenied => "permission-denied",
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorReport {
    pub schema_version: u64,
    pub kind: &'static str,
    pub code: ErrorCode,
    pub exit_status: i32,
    pub message: String,
    pub causes: Vec<String>,
}

pub fn report(error: &Error) -> ErrorReport {
    let message = format!("{error:#}");
    let code = classify_message(&message);
    ErrorReport {
        schema_version: 1,
        kind: "worklouderctl-error",
        code,
        exit_status: code.exit_status(),
        message,
        causes: error.chain().skip(1).map(ToString::to_string).collect(),
    }
}

pub fn classify_message(message: &str) -> ErrorCode {
    let message = message.to_ascii_lowercase();
    if contains_any(
        &message,
        &[
            "status rollback-failed",
            "ended with status rollback-failed",
        ],
    ) {
        ErrorCode::RollbackFailed
    } else if contains_any(
        &message,
        &["status rolled-back", "ended with status rolled-back"],
    ) {
        ErrorCode::OperationRolledBack
    } else if contains_any(
        &message,
        &[
            "permission denied",
            "operation not permitted",
            "accessibility permission",
            "input monitoring permission",
        ],
    ) {
        ErrorCode::PermissionDenied
    } else if contains_any(
        &message,
        &[
            "revision conflict",
            "revision conflicted",
            "source sha-256 conflicted",
            "changed after planning",
            "artifact readback differed",
            "stale compare-and-swap",
            "stale cas",
            "destination appeared during write",
            "did not match this apply retry",
            "did not match this restore retry",
            "differed during transaction postflight",
            "runtime expectation timed out",
        ],
    ) {
        ErrorCode::Conflict
    } else if contains_any(
        &message,
        &[
            "failed to connect",
            "connection refused",
            "bridge is not running",
            "bridge did not become ready",
            "no connected device",
            "provider was unavailable",
            "request timed out",
            "unsupported bridge capability",
            "missing bridge capability",
        ],
    ) {
        ErrorCode::ProviderUnavailable
    } else if contains_any(
        &message,
        &[
            "invalid",
            "unknown",
            "omitted",
            "exceeded",
            "must be",
            "must differ",
            "did not match",
            "mismatched",
            "not found",
            "rejected",
            "unsafe",
            "traversal",
            "was not json",
            "failed to read",
            "failed to inspect",
        ],
    ) {
        ErrorCode::InvalidData
    } else {
        ErrorCode::Unexpected
    }
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_has_stable_precedence_and_statuses() {
        let cases = [
            (
                "ended with status rollback-failed",
                ErrorCode::RollbackFailed,
                7,
            ),
            (
                "ended with status rolled-back",
                ErrorCode::OperationRolledBack,
                6,
            ),
            ("revision conflict", ErrorCode::Conflict, 5),
            ("runtime expectation timed out", ErrorCode::Conflict, 5),
            ("snapshot was invalid JSON", ErrorCode::InvalidData, 4),
            ("bridge is not running", ErrorCode::ProviderUnavailable, 3),
            ("operation not permitted", ErrorCode::PermissionDenied, 8),
            ("unclassified failure", ErrorCode::Unexpected, 1),
        ];
        for (message, expected, status) in cases {
            let actual = classify_message(message);
            assert_eq!(actual, expected);
            assert_eq!(actual.exit_status(), status);
        }
        assert_eq!(SUCCESS, 0);
        assert_eq!(USAGE, 2);
    }
}
