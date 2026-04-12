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

/// Programming language
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Unspecified,
    #[serde(rename = "c")]
    C,
    #[serde(rename = "cpp")]
    Cpp,
    #[serde(rename = "ruc")]
    Ruc,
}

/// Compiler
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub enum Compiler {
    #[default]
    Unspecified,
    #[serde(rename = "gcc")]
    Gcc,
    #[serde(rename = "clang")]
    Clang,
    #[serde(rename = "ruc")]
    Ruc,
    #[serde(rename = "mingw_gcc")]
    MingwGcc,
    #[serde(rename = "mingw_clang")]
    MingwClang,
    #[serde(rename = "msvc")]
    Msvc,
    #[serde(rename = "clang-cl")]
    ClangCl,
}

/// CPU architecture
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub enum Cpu {
    #[default]
    Unspecified,
    #[serde(rename = "x86")]
    X86,
    #[serde(rename = "x86_64")]
    X8664,
    #[serde(rename = "arm")]
    ArmLe,
    #[serde(rename = "arm_be")]
    ArmBe,
    #[serde(rename = "arm64")]
    Arm64Le,
    #[serde(rename = "arm64_be")]
    Arm64Be,
    #[serde(rename = "mipsel")]
    Mips32Le,
    #[serde(rename = "mips")]
    Mips32Be,
    #[serde(rename = "mipsel64")]
    Mips64Le,
    #[serde(rename = "mips64")]
    Mips64Be,
    #[serde(rename = "ruc_vm")]
    RucVm,
}

/// Operating system
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum Os {
    #[default]
    Unspecified,
    Linux,
    Windows,
    Macos,
    Baremetal,
    RucVm,
}

/// Debug verbosity level
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum DebugLevel {
    #[default]
    None,
    Low,
    Medium,
    High,
    Full,
}

/// Kind of analysis job requested
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisJobKind {
    #[default]
    Unknown,
    Defects,
    Identification,
    Dependency,
}

/// Phase of analysis execution (transmitted as integer)
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(try_from = "u32", into = "u32")]
pub enum AnalysisPhase {
    #[default]
    Unknown = 0,
    Pending = 1,
    Starting = 2,
    Parsing = 3,
    Modelling = 4,
    InterproceduralFusion = 5,
    Intraprocedural = 6,
    Interprocedural = 7,
    Finished = 8,
}

impl From<AnalysisPhase> for u32 {
    fn from(p: AnalysisPhase) -> u32 {
        p as u32
    }
}

impl TryFrom<u32> for AnalysisPhase {
    type Error = String;
    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(AnalysisPhase::Unknown),
            1 => Ok(AnalysisPhase::Pending),
            2 => Ok(AnalysisPhase::Starting),
            3 => Ok(AnalysisPhase::Parsing),
            4 => Ok(AnalysisPhase::Modelling),
            5 => Ok(AnalysisPhase::InterproceduralFusion),
            6 => Ok(AnalysisPhase::Intraprocedural),
            7 => Ok(AnalysisPhase::Interprocedural),
            8 => Ok(AnalysisPhase::Finished),
            _ => Ok(AnalysisPhase::Unknown),
        }
    }
}

/// Defect report severity
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum ReportSeverity {
    #[default]
    Unknown,
    Paranoid,
    Low,
    Medium,
    High,
    Critical,
}

impl ReportSeverity {
    /// Parse from SARIF level string
    pub fn from_sarif(s: &str) -> Self {
        match s {
            "note" => ReportSeverity::Medium,
            "warning" => ReportSeverity::High,
            "error" => ReportSeverity::Critical,
            _ => ReportSeverity::Unknown,
        }
    }
}

/// Bitfield of resource type flags
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct ResourceType(pub u32);

impl ResourceType {
    pub const GLOBAL: u32           = 1;
    pub const STATIC: u32           = 2;
    pub const LOCAL: u32            = 4;
    pub const PARAMETER: u32        = 8;
    pub const STRUCTURE_MEMBER: u32 = 16;
    pub const ENUM_MEMBER: u32      = 32;
    pub const METHOD: u32           = 64;
    pub const STANDARD_LIBRARY: u32 = 128;
    pub const NOT_EXISTENT: u32     = 256;
    pub const UNASSIGNED: u32       = 512;
    pub const BUILTIN_LIBRARY: u32  = 1024;

    pub fn from_str(s: &str) -> Self {
        ResourceType(s.parse::<u32>().unwrap_or(0))
    }

    pub fn has(&self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}

/// Origin of a symbol
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum SymbolOrigin {
    #[default]
    Unknown,
    FromSource,
    External,
    StandardLibrary,
    Internal,
}

impl SymbolOrigin {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().replace('-', "_").as_str() {
            "from_source" | "fromsource" => SymbolOrigin::FromSource,
            "external"                   => SymbolOrigin::External,
            "standard_library" | "standardlibrary" => SymbolOrigin::StandardLibrary,
            "internal"                   => SymbolOrigin::Internal,
            _                            => SymbolOrigin::Unknown,
        }
    }
}

/// Bitfield of identification status flags
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct IdentificationStatus(pub u32);

impl IdentificationStatus {
    pub const PARSED: u32       = 1;
    pub const WITH_ERRORS: u32  = 2;
    pub const BEST_MATCH: u32   = 4;

    pub fn has(&self, flag: u32) -> bool {
        self.0 & flag != 0
    }
}