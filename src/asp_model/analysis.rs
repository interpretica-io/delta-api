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
use super::enums::{AnalysisJobKind, AnalysisPhase};

/// A single analysis job specification
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct AnalysisJob {
    pub kind: AnalysisJobKind,
}

impl AnalysisJob {
    pub fn defects()        -> Self { AnalysisJob { kind: AnalysisJobKind::Defects } }
    pub fn identification() -> Self { AnalysisJob { kind: AnalysisJobKind::Identification } }
    pub fn dependency()     -> Self { AnalysisJob { kind: AnalysisJobKind::Dependency } }
}

/// An analysis request / handle
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Analysis {
    /// Server-assigned analysis ID (0xFFFFFFFF = unknown)
    pub id: u32,
    pub jobs: Vec<AnalysisJob>,
}

impl Analysis {
    pub const ID_UNKNOWN: u32 = 0xFFFFFFFF;

    pub fn new() -> Self {
        Analysis {
            id: Self::ID_UNKNOWN,
            jobs: Vec::new(),
        }
    }

    pub fn add_job(mut self, job: AnalysisJob) -> Self {
        self.jobs.push(job);
        self
    }
}

/// Brief analysis descriptor returned by analysis_list
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct AnalysisOutline {
    pub id: u32,
    pub workspace_id: u32,
}

/// Real-time analysis progress state
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct AnalysisState {
    pub current_job: i32,
    pub job_count: i32,
    pub phase: AnalysisPhase,
    pub current: i32,
    pub total: i32,
    /// Percentage progress 0–100
    pub progress: i32,
    pub progress_entity: String,
}