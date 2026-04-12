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

use serde::{Deserialize, Serialize};
use super::report::Report;
use super::symbol::{SymbolCall, SymbolLocation};
use super::identification::IdentificationFile;

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
}

impl AnalysisResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn report_count(&self) -> usize {
        self.reports.len()
    }
}