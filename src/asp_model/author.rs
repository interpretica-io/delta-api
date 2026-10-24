//! Authorship facts, as the `authorship` job reports them.
//!
//! Facts and counts only — there is deliberately no score here. `git blame`
//! names who last touched a line, not who introduced a defect, and commit and
//! line counts measure volume rather than value. Scoring is a consumer's
//! decision to make, and to answer for.
//!
//! `owned_lines` is a fact of the same kind: how many surviving lines `git
//! blame` attributes to an author right now. It is not a ranking — but it is
//! the raw material a consumer needs to answer two structural questions about
//! the *code*, not about the people: how concentrated knowledge is (ownership
//! share) and how few departures would lose it (bus factor). Those derivations
//! are the consumer's to make; this module only reports the line counts.

use serde::{Deserialize, Serialize};

/// One finding class attributed to an author.
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct AuthorFinding {
    /// Rule inner code.
    pub rule: String,
    /// Number of findings of that rule on lines this author last touched.
    pub count: u32,
}

/// One author, as git records them, with what the run attributed to them.
///
/// The commit facts mirror `asp_author`: they are all zero when the history
/// part of the job did not run, which is not the same as an author who made
/// zero commits — the findings list tells the two apart.
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
    /// examined tree — the lines they last touched that are still present.
    /// Unlike `added`/`removed` (churn over history), this is current
    /// ownership, and is what a consumer sums to derive an ownership share and
    /// a bus factor. Zero when the blame/ownership part of the job did not run
    /// (same convention as the commit facts); an author with findings but zero
    /// everything else is that case, not someone who owns no lines.
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
