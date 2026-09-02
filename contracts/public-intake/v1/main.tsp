import "@typespec/http";

using TypeSpec.Http;

@service(#{ title: "Zed public commercial intake v1" })
namespace Zed.PublicIntake.V1;

enum PublicIntakeSchemaV1 {
  v1: "zed.public-intake.v1",
}

enum PublicIntakeSourceHostV1 {
  user: "user.zpkg.net",
  organization: "org.zpkg.net",
}

enum PublicIntakePartyV1 {
  individual,
  organization,
}

enum PublicIntakeInterestV1 {
  packagePublishing: "package_publishing",
  privateRegistry: "private_registry",
  supplyChainSecurity: "supply_chain_security",
  enterpriseSupport: "enterprise_support",
  developerExperience: "developer_experience",
  migration,
  compliance,
  selfHosted: "self_hosted",
  airGapped: "air_gapped",
}

enum QuoteDeploymentModelV1 {
  evaluating,
  zedCloud: "zed_cloud",
  selfHosted: "self_hosted",
  hybrid,
  airGapped: "air_gapped",
}

enum QuoteTeamSizeBandV1 {
  oneToTen: "one_to_ten",
  elevenToFifty: "eleven_to_fifty",
  fiftyOneToTwoHundred: "fifty_one_to_two_hundred",
  twoHundredOneToOneThousand: "two_hundred_one_to_one_thousand",
  overOneThousand: "over_one_thousand",
  unknown,
}

enum QuotePackageCountBandV1 {
  underOneHundred: "under_one_hundred",
  oneHundredToOneThousand: "one_hundred_to_one_thousand",
  oneThousandToTenThousand: "one_thousand_to_ten_thousand",
  overTenThousand: "over_ten_thousand",
  unknown,
}

enum QuoteMonthlyDownloadBandV1 {
  underOneHundredThousand: "under_one_hundred_thousand",
  oneHundredThousandToOneMillion: "one_hundred_thousand_to_one_million",
  oneMillionToTenMillion: "one_million_to_ten_million",
  overTenMillion: "over_ten_million",
  unknown,
}

enum QuoteMigrationWindowV1 {
  exploring,
  underThreeMonths: "under_three_months",
  threeToSixMonths: "three_to_six_months",
  sixToTwelveMonths: "six_to_twelve_months",
  overTwelveMonths: "over_twelve_months",
}

enum PublicIntakeAcceptedStatusV1 {
  accepted,
}

enum PublicIntakeErrorCodeV1 {
  invalidRequest: "invalid_request",
  unsupportedMediaType: "unsupported_media_type",
  payloadTooLarge: "payload_too_large",
  abuseChallengeFailed: "abuse_challenge_failed",
  rateLimited: "rate_limited",
  temporarilyUnavailable: "temporarily_unavailable",
}

model PreInterestRegistrationRequestV1 {
  schema: PublicIntakeSchemaV1;
  requestId: string;
  email: string;
  partyType: PublicIntakePartyV1;
  sourceHost: PublicIntakeSourceHostV1;
  interests: PublicIntakeInterestV1[];
  contactName?: string;
  organizationName?: string;
  websiteUrl?: string;
  locale?: string;
  referralCode?: string;
  consentRevision: string;
  consentedAt: string;
  contactConsent: boolean;
  marketingConsent: boolean;
  marketingConsentRevision?: string;
}

model QuoteRequestV1 {
  schema: PublicIntakeSchemaV1;
  requestId: string;
  email: string;
  sourceHost: PublicIntakeSourceHostV1;
  organizationName: string;
  contactName: string;
  role?: string;
  websiteUrl?: string;
  interests: PublicIntakeInterestV1[];
  deploymentModel: QuoteDeploymentModelV1;
  teamSize: QuoteTeamSizeBandV1;
  packageCount: QuotePackageCountBandV1;
  monthlyDownloads: QuoteMonthlyDownloadBandV1;
  migrationWindow: QuoteMigrationWindowV1;
  requirementsSummary?: string;
  locale?: string;
  referralCode?: string;
  consentRevision: string;
  consentedAt: string;
  contactConsent: boolean;
  marketingConsent: boolean;
  marketingConsentRevision?: string;
}

model PublicIntakeAcceptedV1 {
  schema: PublicIntakeSchemaV1;
  status: PublicIntakeAcceptedStatusV1;
}

model PublicIntakeErrorV1 {
  schema: PublicIntakeSchemaV1;
  code: PublicIntakeErrorCodeV1;
  retryable: boolean;
}

@route("/v1/pre-interest")
@post
op submitPreInterest(@body request: PreInterestRegistrationRequestV1): PublicIntakeAcceptedV1 | PublicIntakeErrorV1;

@route("/v1/quote-requests")
@post
op submitQuoteRequest(@body request: QuoteRequestV1): PublicIntakeAcceptedV1 | PublicIntakeErrorV1;
