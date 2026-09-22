//! Package-version retirement contract.
//!
//! Retirement is deliberately orthogonal to yanking. A retired version remains
//! resolvable and downloadable, but clients should surface its reason/message.
//! A yanked version is excluded from fresh resolution. This mirrors the useful
//! lifecycle distinction made by Hex while keeping the contract ecosystem-neutral.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const MAX_RETIREMENT_MESSAGE_CHARS: usize = 140;
pub const RETIRE_AUDIT_ACTION: &str = "retire";
pub const UNRETIRE_AUDIT_ACTION: &str = "unretire";

/// `POST` (bearer token, org-scoped) — retire or unretire one immutable release.
pub fn retirement_path(org: &str, name: &str, version: &str) -> String {
    format!("/v1/packages/{org}/{name}/versions/{version}/retirement")
}

/// Stable retirement reasons. These intentionally match the lifecycle reasons
/// exposed by Hex so BEAM clients can preserve semantics without translation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RetirementReason {
    Renamed,
    Deprecated,
    Security,
    Invalid,
    Other,
}

impl RetirementReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Renamed => "renamed",
            Self::Deprecated => "deprecated",
            Self::Security => "security",
            Self::Invalid => "invalid",
            Self::Other => "other",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "renamed" => Some(Self::Renamed),
            "deprecated" => Some(Self::Deprecated),
            "security" => Some(Self::Security),
            "invalid" => Some(Self::Invalid),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// Advisory lifecycle metadata attached to a published version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Retirement {
    pub reason: RetirementReason,
    /// Human-readable explanation. Hex specifies a 140-character maximum;
    /// count Unicode scalar values rather than UTF-8 bytes so non-ASCII text is
    /// not rejected more aggressively than the compatibility target.
    pub message: String,
}

impl Retirement {
    pub fn validate(&self) -> Result<(), RetirementError> {
        if self.message.trim().is_empty() {
            return Err(RetirementError::EmptyMessage);
        }
        let characters = self.message.chars().count();
        if characters > MAX_RETIREMENT_MESSAGE_CHARS {
            return Err(RetirementError::MessageTooLong {
                actual: characters,
                maximum: MAX_RETIREMENT_MESSAGE_CHARS,
            });
        }
        Ok(())
    }
}

/// Body for the retirement route. `retirement: null` restores a retired
/// version without changing its immutable artifact or yank state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetireRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirement: Option<Retirement>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RetireResponse {
    pub org: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirement: Option<Retirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetirementError {
    EmptyMessage,
    MessageTooLong { actual: usize, maximum: usize },
}

impl std::fmt::Display for RetirementError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyMessage => f.write_str("retirement message must not be empty"),
            Self::MessageTooLong { actual, maximum } => write!(
                f,
                "retirement message is {actual} characters; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for RetirementError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_is_version_scoped() {
        assert_eq!(
            retirement_path("acme", "beam_lib", "1.2.3"),
            "/v1/packages/acme/beam_lib/versions/1.2.3/retirement"
        );
    }

    #[test]
    fn all_hex_reasons_have_stable_wire_names_and_parse_back() {
        let cases = [
            (RetirementReason::Renamed, "renamed"),
            (RetirementReason::Deprecated, "deprecated"),
            (RetirementReason::Security, "security"),
            (RetirementReason::Invalid, "invalid"),
            (RetirementReason::Other, "other"),
        ];
        for (reason, expected) in cases {
            assert_eq!(reason.as_str(), expected);
            assert_eq!(RetirementReason::parse(expected), Some(reason));
            let rendered = serde_json::to_string(&reason);
            assert!(matches!(
                rendered,
                Ok(ref value) if value == &format!("\"{expected}\"")
            ));
        }
        assert_eq!(RetirementReason::parse("unknown"), None);
    }

    #[test]
    fn retirement_message_is_bounded_by_characters() {
        assert_eq!(
            Retirement {
                reason: RetirementReason::Invalid,
                message: "does not compile on OTP 29".into(),
            }
            .validate(),
            Ok(())
        );

        assert_eq!(
            Retirement {
                reason: RetirementReason::Other,
                message: " ".into(),
            }
            .validate(),
            Err(RetirementError::EmptyMessage)
        );

        assert_eq!(
            Retirement {
                reason: RetirementReason::Deprecated,
                message: "é".repeat(MAX_RETIREMENT_MESSAGE_CHARS),
            }
            .validate(),
            Ok(())
        );

        assert!(matches!(
            Retirement {
                reason: RetirementReason::Security,
                message: "x".repeat(MAX_RETIREMENT_MESSAGE_CHARS + 1),
            }
            .validate(),
            Err(RetirementError::MessageTooLong { .. })
        ));
    }

    #[test]
    fn null_retirement_means_unretire() {
        let parsed: Result<RetireRequest, _> = serde_json::from_str(r#"{"retirement":null}"#);
        assert!(matches!(parsed, Ok(RetireRequest { retirement: None })));
    }
}
