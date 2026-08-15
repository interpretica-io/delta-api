//! Per-function code metrics, as the `metrics` job reports them.
//!
//! One row per function, numbers only. Aggregates are deliberately absent:
//! a median carried over the wire would be wrong the moment two results are
//! combined, so summarising is the consumer's to do. When ranking "worst
//! first", cognitive complexity is the honest key — cyclomatic rewards a
//! flat switch, cognitive weighs the nesting a change actually costs.

use serde::{Deserialize, Serialize};

/// Metrics of one function, mirroring `asp_function_metrics`.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct FunctionMetrics {
    /// Function name.
    pub name: String,
    /// File it is defined in.
    pub file: String,
    /// Line it starts on.
    pub line: u32,
    /// Lines in the body; 0 when the backend tracks no ends.
    pub lines: u32,
    /// Ways through (McCabe).
    pub cyclomatic: u32,
    /// Branches weighted by nesting depth.
    pub cognitive: u32,
    /// Deepest nesting.
    pub nesting: u32,
    /// Declared parameters.
    pub parameters: u32,
    /// Statements at any depth.
    pub statements: u32,
    /// Points it returns from.
    pub returns: u32,
}
