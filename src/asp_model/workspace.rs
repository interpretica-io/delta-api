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
use super::env::Environment;
use super::file::File;

/// Workspace (a named collection of files with an environment)
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Workspace {
    /// Server-assigned workspace ID (0xFFFFFFFF = unknown)
    pub id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<Environment>,
    #[serde(default)]
    pub files: Vec<File>,
}

impl Workspace {
    pub const ID_UNKNOWN: u32 = 0xFFFFFFFF;

    pub fn new(name: impl Into<String>) -> Self {
        Workspace {
            id: Self::ID_UNKNOWN,
            name: Some(name.into()),
            env: None,
            files: Vec::new(),
        }
    }

    pub fn with_env(mut self, env: Environment) -> Self {
        self.env = Some(env);
        self
    }

    pub fn add_file(&mut self, file: File) {
        self.files.push(file);
    }
}

/// Brief workspace descriptor returned by workspace_list
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct WorkspaceOutline {
    pub id: u32,
    pub name: Option<String>,
}