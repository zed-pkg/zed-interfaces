//! Additional byte representations for `zpkg/dependency-graph/v1`.
//!
//! The graph document and its semantic digest are defined in
//! [`crate::dependency_graph`]. This module defines transport-level download
//! representations that are projected from one finalized document. It does not
//! add another graph model and it never authorizes a server to re-resolve an
//! immutable package version.

/// Response header that distinguishes lossless interchange formats from
/// convenience projections such as CSV.
pub const DEPENDENCY_GRAPH_AUTHORITATIVE_HEADER: &str = "x-zpkg-graph-authoritative";

pub const DEPENDENCY_GRAPH_JSON5_MEDIA_TYPE: &str =
    "application/vnd.zpkg.dependency-graph.v1+json5";
pub const DEPENDENCY_GRAPH_XML_MEDIA_TYPE: &str =
    "application/vnd.zpkg.dependency-graph.v1+xml";
pub const DEPENDENCY_GRAPH_CSV_MEDIA_TYPE: &str = "text/csv; charset=utf-8";
pub const DEPENDENCY_GRAPH_MESSAGEPACK_MEDIA_TYPE: &str =
    "application/vnd.zpkg.dependency-graph.v1+msgpack";
pub const DEPENDENCY_GRAPH_PROTOBUF_MEDIA_TYPE: &str =
    "application/vnd.zpkg.dependency-graph.v1+protobuf";

/// Additive route for representations that are not part of the original v1
/// query-format enum. Existing JSON/YAML/TOML/DOT/Mermaid URLs remain stable.
pub const DEPENDENCY_GRAPH_EXPORT_ROUTE_TEMPLATE: &str =
    "/v1/packages/{org}/{name}/versions/{version}/dependency-graph/export/{format}";

/// Additional dependency-graph download representations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DependencyGraphExportFormat {
    Json5,
    Xml,
    Csv,
    MessagePack,
    Protobuf,
}

impl DependencyGraphExportFormat {
    pub const ALL: [Self; 5] = [
        Self::Json5,
        Self::Xml,
        Self::Csv,
        Self::MessagePack,
        Self::Protobuf,
    ];

    /// Canonical spelling used in URLs and download controls.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Json5 => "json5",
            Self::Xml => "xml",
            Self::Csv => "csv",
            Self::MessagePack => "msgpack",
            Self::Protobuf => "protobuf",
        }
    }

    /// Parse canonical names plus common tool aliases.
    pub fn parse_name(value: &str) -> Option<Self> {
        Some(match value.to_ascii_lowercase().as_str() {
            "json5" => Self::Json5,
            "xml" => Self::Xml,
            "csv" => Self::Csv,
            "msgpack" | "messagepack" | "mpk" => Self::MessagePack,
            "protobuf" | "proto" | "pb" => Self::Protobuf,
            _ => return None,
        })
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Json5 => "json5",
            Self::Xml => "xml",
            Self::Csv => "csv",
            Self::MessagePack => "msgpack",
            Self::Protobuf => "pb",
        }
    }

    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Json5 => DEPENDENCY_GRAPH_JSON5_MEDIA_TYPE,
            Self::Xml => DEPENDENCY_GRAPH_XML_MEDIA_TYPE,
            Self::Csv => DEPENDENCY_GRAPH_CSV_MEDIA_TYPE,
            Self::MessagePack => DEPENDENCY_GRAPH_MESSAGEPACK_MEDIA_TYPE,
            Self::Protobuf => DEPENDENCY_GRAPH_PROTOBUF_MEDIA_TYPE,
        }
    }

    /// CSV is a flat analytics projection. All other formats retain every v1
    /// graph field and can reconstruct the finalized document.
    pub const fn is_authoritative(self) -> bool {
        !matches!(self, Self::Csv)
    }

    pub const fn is_binary(self) -> bool {
        matches!(self, Self::MessagePack | Self::Protobuf)
    }
}

/// Build the additive declared-graph export path. Coordinates are expected to
/// have already passed the same registry validation used by the canonical path
/// helper.
pub fn declared_dependency_graph_export_path(
    org: &str,
    name: &str,
    version: &str,
    format: DependencyGraphExportFormat,
) -> String {
    format!(
        "/v1/packages/{org}/{name}/versions/{version}/dependency-graph/export/{}",
        format.name()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_names_round_trip() {
        for format in DependencyGraphExportFormat::ALL {
            assert_eq!(
                DependencyGraphExportFormat::parse_name(format.name()),
                Some(format)
            );
            assert!(!format.extension().is_empty());
            assert!(!format.media_type().is_empty());
        }
    }

    #[test]
    fn common_binary_aliases_are_supported() {
        assert_eq!(
            DependencyGraphExportFormat::parse_name("messagepack"),
            Some(DependencyGraphExportFormat::MessagePack)
        );
        assert_eq!(
            DependencyGraphExportFormat::parse_name("mpk"),
            Some(DependencyGraphExportFormat::MessagePack)
        );
        assert_eq!(
            DependencyGraphExportFormat::parse_name("proto"),
            Some(DependencyGraphExportFormat::Protobuf)
        );
        assert_eq!(
            DependencyGraphExportFormat::parse_name("PB"),
            Some(DependencyGraphExportFormat::Protobuf)
        );
    }

    #[test]
    fn authority_and_binary_classification_is_explicit() {
        assert!(!DependencyGraphExportFormat::Csv.is_authoritative());
        assert!(DependencyGraphExportFormat::Xml.is_authoritative());
        assert!(DependencyGraphExportFormat::MessagePack.is_binary());
        assert!(!DependencyGraphExportFormat::Json5.is_binary());
    }

    #[test]
    fn path_uses_canonical_format_name() {
        assert_eq!(
            declared_dependency_graph_export_path(
                "acme",
                "widget",
                "1.2.0-beta.1",
                DependencyGraphExportFormat::Protobuf,
            ),
            "/v1/packages/acme/widget/versions/1.2.0-beta.1/dependency-graph/export/protobuf"
        );
    }
}
