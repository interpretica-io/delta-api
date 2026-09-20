//! Suspicious places, as the analyser reports them beside the findings.
//!
//! A finding states a defect. A suspicious place states that the code needs a
//! reader: the two arms of a branch are identical, this is the one call site
//! that drops the result, this function decides an access. The analyser keeps
//! the two apart all the way down, and so does this crate — a consumer that
//! counts findings must not accidentally count these, because a suspicious
//! place is not a claim that anything is wrong.
//!
//! The category, the confidence and the rule travel as strings rather than as
//! enumerations. The analyser grows rules faster than its clients are rebuilt,
//! and a client built before a category existed should still be able to show
//! the entry and let a person judge it.

use serde::{Deserialize, Serialize};

/// One line of the code around a suspicious place.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct SuspicionLine {
    /// Line number, counting from one.
    pub line: u32,
    /// The line itself, without its terminator.
    pub text: String,
    /// `true` when the line is part of the construct rather than context
    /// around it.
    #[serde(default)]
    pub marked: bool,
}

/// One place the analyser wants a person to look at.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Suspicion {
    /// Kind of observation: `security`, `copy-paste`, `boundary`, …
    pub category: String,
    /// `high`, `medium` or `low`. `low` is inventory — the analyser only
    /// reports it when asked for everything.
    pub confidence: String,
    /// Rule slug, e.g. `password-check`.
    pub rule: String,
    /// Full rule id, e.g. `visao/suspicious/security/password-check`.
    pub rule_id: String,
    /// What was observed, in one line.
    pub title: String,
    /// Why it looks unusual.
    #[serde(default)]
    pub odd: String,
    /// What a reader is being asked to confirm.
    #[serde(default)]
    pub check: String,
    /// File it sits in, as the analyser named it.
    pub file: String,
    /// First line of the construct.
    pub line: u32,
    /// Last line of it; equal to `line` for a one-liner.
    #[serde(default)]
    pub end_line: u32,
    /// Column it starts at.
    #[serde(default)]
    pub column: u32,
    /// The code around it. Empty when the analyser could not read the file
    /// back off disk, which is normal rather than an error.
    #[serde(default)]
    pub snippet: Vec<SuspicionLine>,
}

impl Suspicion {
    /// `true` for the category whose entries are not "this looks odd" but
    /// "this decides something, and a reader has to confirm the decision".
    pub fn is_security(&self) -> bool {
        self.category == "security"
    }

    /// `true` when the analyser rates the place worth a reader's time without
    /// being asked for the full inventory.
    pub fn is_confident(&self) -> bool {
        self.confidence == "high" || self.confidence == "medium"
    }
}
