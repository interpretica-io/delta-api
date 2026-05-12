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

use super::address::Address;
use super::env::Environment;
use serde::{Deserialize, Serialize};

/// A source file with optional per-file environment override
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct File {
    pub address: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Environment>,
}

impl File {
    /// Create a file from a local path
    pub fn from_path(path: impl Into<String>) -> Self {
        File {
            address: Address::local_file(path),
            env: None,
        }
    }

    /// Create a file with a per-file environment
    pub fn with_env(mut self, env: Environment) -> Self {
        self.env = Some(env);
        self
    }
}
