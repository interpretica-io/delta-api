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
pub struct SubjectAliveStatus {
    pub alive: bool,
    pub bind_addr: String,
    pub bind_port: u16,
}

impl SubjectAliveStatus {
    pub fn new() -> SubjectAliveStatus {
        return SubjectAliveStatus {
            alive: false,
            bind_addr: "".to_string(),
            bind_port: 0,
        };
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct ConnAliveStatus {
    pub subjects: HashMap<DeploySubject, SubjectAliveStatus>,
}

impl ConnAliveStatus {
    pub fn new() -> ConnAliveStatus {
        return ConnAliveStatus {
            subjects: HashMap::new()
        };
    }
}
