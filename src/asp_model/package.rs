//! The project's declared dependencies, as the `sbom` job found them — the
//! same knowledge the CycloneDX document carries, for a client that cannot
//! read a file on the analyser's machine.

use serde::{Deserialize, Serialize};

/// One advisory matched against a package.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct VulnerabilityInfo {
    /// CVE / GHSA / OSV identifier.
    pub id: String,
    /// Database it came from.
    #[serde(default)]
    pub source: String,
    /// Severity, as the advisory database spelled it — free-form.
    #[serde(default)]
    pub severity: String,
    /// Short summary.
    #[serde(default)]
    pub summary: String,
}

/// One package the project depends on.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct PackageInfo {
    /// Package name.
    pub name: String,
    /// Best known version: resolved when a lock file was found, otherwise
    /// what the manifest spelled — which may be a range.
    #[serde(default)]
    pub version: String,
    /// The constraint the manifest asked for, when that is not the same as
    /// a version.
    #[serde(default)]
    pub declared: String,
    /// Whether `version` came from a lock file.
    #[serde(default)]
    pub resolved: bool,
    /// `cargo`, `npm`, `pip`, …
    #[serde(default)]
    pub ecosystem: String,
    /// Package URL.
    #[serde(default)]
    pub purl: String,
    /// Declared license, may be empty.
    #[serde(default)]
    pub license: String,
    /// Manifest that declared it.
    #[serde(default)]
    pub source: String,
    /// A workspace member rather than an external dependency.
    #[serde(default)]
    pub first_party: bool,
    /// Advisories matched against this package.
    #[serde(default)]
    pub vulnerabilities: Vec<VulnerabilityInfo>,
}
