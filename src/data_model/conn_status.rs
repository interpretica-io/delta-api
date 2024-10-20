/*
 * Delta API
 *
 * Copyright 2024 Maxim Menshikov
 *
 * Permission is hereby granted, free of charge, to any person obtaining
 * a copy of this software and associated documentation files (the “Software”),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit persons to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included
 * in all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED “AS IS”, WITHOUT WARRANTY OF ANY KIND, EXPRESS
 * OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

use crate::data_model::deploy_subject::DeploySubject;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct SubjectStatus {
    pub deploy_archive_copied: bool,
    pub deploy_archive_extracted: bool,
    pub deploy_archive_tested: bool,
    pub deployed: bool,
    pub running: bool,
}

impl SubjectStatus {
    pub fn new() -> SubjectStatus {
        return SubjectStatus {
            deploy_archive_copied: false,
            deploy_archive_extracted: false,
            deploy_archive_tested: false,
            deployed: false,
            running: false,
        };
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct ConnStatus {
    pub connected: bool,
    pub subjects: HashMap<DeploySubject, SubjectStatus>,
    pub platform: String,
}

impl ConnStatus {
    pub fn new(connected: bool) -> ConnStatus {
        return ConnStatus { connected: connected,
            subjects: HashMap::new(),
            platform: "".to_string() }
    }

    pub fn get_subject(&mut self, subject: DeploySubject) -> SubjectStatus {
        if !self.subjects.contains_key(&subject) {
            return SubjectStatus::new();
        }

        return self.subjects[&subject].clone();
    }

    pub fn set_subject(&mut self, subject: DeploySubject, status: SubjectStatus) {
        if self.subjects.contains_key(&subject) {
            let m = self.subjects.get_mut(&subject);
            *m.unwrap() = status;
        } else {
            self.subjects.insert(subject, status);
        }
    }
}