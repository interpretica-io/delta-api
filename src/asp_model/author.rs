//! Authorship facts, as the `authorship` job reports them.
//!
//! The job reports facts and counts. It does not score authors. `git blame`
//! names who last touched a line. It does not name who introduced a defect.
//! Commit and line counts measure volume. A consumer that wants a score must
//! compute it and answer for it.
//!
//! `owned_lines` is one such fact: the surviving lines `git blame` attributes
//! to an author now. A consumer sums it to derive two properties of the code:
//! the ownership share, and the bus factor (how few authors hold a given
//! share). This module reports the line counts only.

use serde::{Deserialize, Serialize};

/// One finding class attributed to an author.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct AuthorFinding {
    /// Rule inner code.
    pub rule: String,
    /// Number of findings of that rule on lines this author last touched.
    pub count: u32,
}

/// One author as git records them, with what the run attributed to them.
///
/// The commit facts mirror `asp_author`. They are all zero when the history
/// part of the job did not run. An author who made zero commits looks the
/// same, so use the findings list to tell the two apart.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct AuthorInfo {
    /// Author name as git records it.
    pub name: String,
    /// Findings on lines this author last touched, by rule.
    #[serde(default)]
    pub findings: Vec<AuthorFinding>,
    /// Commits in the examined range.
    #[serde(default)]
    pub commits: u32,
    /// Distinct days committed on.
    #[serde(default)]
    pub days: u32,
    /// Lines added.
    #[serde(default)]
    pub added: u32,
    /// Lines removed.
    #[serde(default)]
    pub removed: u32,
    /// Surviving lines `git blame` attributes to this author across the
    /// examined tree. These are the lines they last touched that still exist.
    /// `added`/`removed` count churn over history; this counts current
    /// ownership. A consumer sums it to derive the ownership share and the bus
    /// factor. It is zero when the blame part of the job did not run, the same
    /// convention as the commit facts.
    #[serde(default)]
    pub owned_lines: u32,
    /// File changes across commits.
    #[serde(default)]
    pub file_changes: u32,
    /// Largest commit, lines added.
    #[serde(default)]
    pub largest_commit: u32,
    /// Median commit, lines added.
    #[serde(default)]
    pub median_commit: u32,
    /// Commits on the busiest day.
    #[serde(default)]
    pub busiest_day: u32,
    /// Commits whose message declares an assistant co-author.
    #[serde(default)]
    pub declared_assistant: u32,
    /// Commits whose region a later commit worked again.
    #[serde(default)]
    pub reworked: u32,
    /// Commits a later fix-shaped commit touched the same file as.
    #[serde(default)]
    pub followed_by_fix: u32,
}
