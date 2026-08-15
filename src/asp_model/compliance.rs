//! Coding-standard compliance, as the analysis reports it.
//!
//! One row per rule of the standard — including the rules the analyser does
//! not check, whose `checks` is empty. That row is the point of the section:
//! a listing of violations alone lets a reader mistake an unchecked rule for
//! a satisfied one.

use serde::{Deserialize, Serialize};

/// One rule of the enforced standard, mirroring `asp_compliance_rule`.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct ComplianceRule {
    /// Rule identifier within the standard.
    pub rule: String,
    /// Mandatory / Required / Advisory.
    pub category: String,
    /// What the analyser checks for it; empty when it checks nothing.
    pub checks: String,
    /// Findings attributed to this rule in the run.
    pub findings: u32,
}
