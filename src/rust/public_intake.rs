//! Versioned public commercial-intake contracts shared by the Zed web, API,
//! Flutter, and browser clients.
//!
//! These types deliberately contain no persistence, networking, challenge
//! verification, or logging behavior. The API boundary validates the request,
//! verifies abuse controls, derives idempotency, and persists through the
//! reviewed product-owned data layer. Accepted and error envelopes contain no
//! submitted contact data, so a public response cannot reflect PII by shape.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Exact wire-contract marker for every v1 public-intake document.
pub const PUBLIC_INTAKE_SCHEMA_V1: &str = "zed.public-intake.v1";

/// Individual pre-interest registration endpoint.
pub const PRE_INTEREST_PATH_V1: &str = "/v1/pre-interest";

/// Organization quote-request endpoint.
pub const QUOTE_REQUESTS_PATH_V1: &str = "/v1/quote-requests";

/// Standard idempotency header accepted by both write routes.
pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

/// Challenge proof header for JSON clients. Browser form submissions may use
/// the provider's ordinary form field and the API adapter maps it internally.
pub const ABUSE_PROOF_HEADER: &str = "X-Zed-Abuse-Proof";

/// A single-variant enum makes the schema marker closed in Rust, JSON Schema,
/// Dart, and TypeScript instead of accepting an arbitrary version string.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema,
)]
pub enum PublicIntakeSchemaV1 {
    #[default]
    #[serde(rename = "zed.public-intake.v1")]
    V1,
}

/// Public browser roles admitted by the v1 contract. The domain layer also
/// checks that an individual request uses `user.zpkg.net` and an organization
/// request uses `org.zpkg.net`; this enum prevents nonstandard hostnames from
/// entering the shared wire model at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum PublicIntakeSourceHostV1 {
    #[serde(rename = "user.zpkg.net")]
    User,
    #[serde(rename = "org.zpkg.net")]
    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicIntakePartyV1 {
    Individual,
    Organization,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicIntakeInterestV1 {
    PackagePublishing,
    PrivateRegistry,
    SupplyChainSecurity,
    EnterpriseSupport,
    DeveloperExperience,
    Migration,
    Compliance,
    SelfHosted,
    AirGapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteDeploymentModelV1 {
    Evaluating,
    ZedCloud,
    SelfHosted,
    Hybrid,
    AirGapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteTeamSizeBandV1 {
    OneToTen,
    ElevenToFifty,
    FiftyOneToTwoHundred,
    TwoHundredOneToOneThousand,
    OverOneThousand,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuotePackageCountBandV1 {
    UnderOneHundred,
    OneHundredToOneThousand,
    OneThousandToTenThousand,
    OverTenThousand,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteMonthlyDownloadBandV1 {
    UnderOneHundredThousand,
    OneHundredThousandToOneMillion,
    OneMillionToTenMillion,
    OverTenMillion,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QuoteMigrationWindowV1 {
    Exploring,
    UnderThreeMonths,
    ThreeToSixMonths,
    SixToTwelveMonths,
    OverTwelveMonths,
}

/// Register interest without creating an account, quote, organization, or
/// authenticated session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreInterestRegistrationRequestV1 {
    pub schema: PublicIntakeSchemaV1,
    /// Client-generated UUID used together with `Idempotency-Key`. It is not
    /// returned from the public endpoint.
    #[schemars(length(min = 36, max = 36))]
    pub request_id: String,
    #[schemars(length(min = 3, max = 254))]
    pub email: String,
    pub party_type: PublicIntakePartyV1,
    pub source_host: PublicIntakeSourceHostV1,
    #[schemars(length(min = 1, max = 9))]
    pub interests: Vec<PublicIntakeInterestV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 120))]
    pub contact_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 200))]
    pub organization_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 8, max = 2048))]
    pub website_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 2, max = 35))]
    pub locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub referral_code: Option<String>,
    #[schemars(length(min = 1, max = 64))]
    pub consent_revision: String,
    /// RFC 3339 timestamp supplied by the browser and bounded by domain
    /// validation against server time.
    #[schemars(length(min = 20, max = 35))]
    pub consented_at: String,
    /// Must be true after validation. This is consent to store and contact,
    /// not account creation and not marketing consent.
    pub contact_consent: bool,
    pub marketing_consent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub marketing_consent_revision: Option<String>,
}

/// Request organization pricing. This stays a separate intent from
/// pre-interest registration, so registration can never silently become a
/// quote request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuoteRequestV1 {
    pub schema: PublicIntakeSchemaV1,
    #[schemars(length(min = 36, max = 36))]
    pub request_id: String,
    #[schemars(length(min = 3, max = 254))]
    pub email: String,
    pub source_host: PublicIntakeSourceHostV1,
    #[schemars(length(min = 1, max = 200))]
    pub organization_name: String,
    #[schemars(length(min = 1, max = 120))]
    pub contact_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 120))]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 8, max = 2048))]
    pub website_url: Option<String>,
    #[schemars(length(min = 1, max = 9))]
    pub interests: Vec<PublicIntakeInterestV1>,
    pub deployment_model: QuoteDeploymentModelV1,
    pub team_size: QuoteTeamSizeBandV1,
    pub package_count: QuotePackageCountBandV1,
    pub monthly_downloads: QuoteMonthlyDownloadBandV1,
    pub migration_window: QuoteMigrationWindowV1,
    /// Intentionally bounded and optional. The form warns submitters not to
    /// include credentials, private keys, tokens, or regulated data; the API
    /// must never log this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 1000))]
    pub requirements_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 2, max = 35))]
    pub locale: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub referral_code: Option<String>,
    #[schemars(length(min = 1, max = 64))]
    pub consent_revision: String,
    #[schemars(length(min = 20, max = 35))]
    pub consented_at: String,
    pub contact_consent: bool,
    pub marketing_consent: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(min = 1, max = 64))]
    pub marketing_consent_revision: Option<String>,
}

/// A generic 202 response. It cannot disclose whether an email, organization,
/// or idempotency key was already known because none of those fields exist in
/// the response model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicIntakeAcceptedV1 {
    pub schema: PublicIntakeSchemaV1,
    pub status: PublicIntakeAcceptedStatusV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicIntakeAcceptedStatusV1 {
    Accepted,
}

/// Closed public error categories. No arbitrary detail or reflected input is
/// present in the wire shape; operational detail belongs only in redacted
/// internal telemetry keyed by a server-generated trace identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublicIntakeErrorV1 {
    pub schema: PublicIntakeSchemaV1,
    pub code: PublicIntakeErrorCodeV1,
    pub retryable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PublicIntakeErrorCodeV1 {
    InvalidRequest,
    UnsupportedMediaType,
    PayloadTooLarge,
    AbuseChallengeFailed,
    RateLimited,
    TemporarilyUnavailable,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pre_interest_request() -> PreInterestRegistrationRequestV1 {
        PreInterestRegistrationRequestV1 {
            schema: PublicIntakeSchemaV1::V1,
            request_id: "018f5f52-feb8-7d4a-a9d6-69d8a1559e8b".to_owned(),
            email: "person@example.com".to_owned(),
            party_type: PublicIntakePartyV1::Individual,
            source_host: PublicIntakeSourceHostV1::User,
            interests: vec![PublicIntakeInterestV1::DeveloperExperience],
            contact_name: None,
            organization_name: None,
            website_url: None,
            locale: Some("en-US".to_owned()),
            referral_code: None,
            consent_revision: "privacy-2026-09-01".to_owned(),
            consented_at: "2026-09-01T21:00:00Z".to_owned(),
            contact_consent: true,
            marketing_consent: false,
            marketing_consent_revision: None,
        }
    }

    #[test]
    fn request_contract_rejects_unknown_fields() {
        let mut value = serde_json::to_value(pre_interest_request()).expect("request serializes");
        value
            .as_object_mut()
            .expect("request is an object")
            .insert("admin".to_owned(), json!(true));

        let error = serde_json::from_value::<PreInterestRegistrationRequestV1>(value)
            .expect_err("unknown field must fail closed");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn accepted_response_has_no_contact_or_request_identity_fields() {
        let value = serde_json::to_value(PublicIntakeAcceptedV1 {
            schema: PublicIntakeSchemaV1::V1,
            status: PublicIntakeAcceptedStatusV1::Accepted,
        })
        .expect("accepted response serializes");

        assert_eq!(
            value,
            json!({
                "schema": PUBLIC_INTAKE_SCHEMA_V1,
                "status": "accepted"
            })
        );
    }

    #[test]
    fn standard_source_hosts_have_exact_wire_values() {
        assert_eq!(
            serde_json::to_value(PublicIntakeSourceHostV1::User).unwrap(),
            json!("user.zpkg.net")
        );
        assert_eq!(
            serde_json::to_value(PublicIntakeSourceHostV1::Organization).unwrap(),
            json!("org.zpkg.net")
        );
    }

    #[test]
    fn public_write_paths_are_stable() {
        assert_eq!(PRE_INTEREST_PATH_V1, "/v1/pre-interest");
        assert_eq!(QUOTE_REQUESTS_PATH_V1, "/v1/quote-requests");
        assert_eq!(IDEMPOTENCY_KEY_HEADER, "Idempotency-Key");
    }
}
