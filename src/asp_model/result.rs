/*
 * Delta API
 *
 * Copyright 2024 Maxim Menshikov
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

use super::author::AuthorInfo;
use super::compliance::ComplianceRule;
use super::identification::IdentificationFile;
use super::metrics::FunctionMetrics;
use super::package::PackageInfo;
use super::report::Report;
use super::symbol::{SymbolCall, SymbolLocation};
use serde::{Deserialize, Serialize};

/// Complete analysis result
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct AnalysisResult {
    /// Raw SARIF JSON text (if available)
    pub sarif_text: Option<String>,
    /// Defect reports
    pub reports: Vec<Report>,
    /// Per-file symbol locations
    pub symbol_data: Vec<SymbolLocation>,
    /// Connected-area symbol locations
    pub connected_symbol_data: Vec<SymbolLocation>,
    /// Symbol call map
    pub symbol_calls: Vec<SymbolCall>,
    /// Language identification results
    pub identification_files: Vec<IdentificationFile>,
    /// Authorship facts, when the `authorship` job ran
    #[serde(default)]
    pub authors: Vec<AuthorInfo>,
    /// Declared dependencies, when the `sbom` job ran
    #[serde(default)]
    pub packages: Vec<PackageInfo>,
    /// One row per rule of the enforced standard, when one was enforced —
    /// including the rules the analyser does not check (`checks` empty).
    #[serde(default)]
    pub compliance: Vec<ComplianceRule>,
    /// The enforced standard's token (e.g. `misra-c-2012`), alongside
    /// `compliance`; empty when no standard was set.
    #[serde(default)]
    pub compliance_standard: String,
    /// Per-function code metrics, when the `metrics` job ran.
    #[serde(default)]
    pub metrics: Vec<FunctionMetrics>,
}

impl AnalysisResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn report_count(&self) -> usize {
        self.reports.len()
    }
}
