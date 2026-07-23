use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// On-the-wire formats for published package artifacts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum ArtifactFormat {
    #[default]
    #[serde(rename = "tar.gz")]
    TarGz,
    #[serde(rename = "zip")]
    Zip,
}

impl ArtifactFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            ArtifactFormat::TarGz => "tar.gz",
            ArtifactFormat::Zip => "zip",
        }
    }

    pub fn content_type(&self) -> &'static str {
        match self {
            ArtifactFormat::TarGz => "application/gzip",
            ArtifactFormat::Zip => "application/zip",
        }
    }
}

impl fmt::Display for ArtifactFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.extension())
    }
}
