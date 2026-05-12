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
use super::enums::{Compiler, Cpu, Language, Os};
use serde::{Deserialize, Serialize};

/// A single include directory entry
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub struct IncludeDirectory {
    pub address: Address,
}

/// Analysis environment (compiler toolchain configuration)
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Environment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compiler: Option<Compiler>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<Cpu>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_os: Option<Os>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub windows_kit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vctools: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub include_dirs: Vec<IncludeDirectory>,
    /// Override environment from higher levels
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub override_higher_level: bool,
}

impl Environment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_language(mut self, lang: Language) -> Self {
        self.language = Some(lang);
        self
    }

    pub fn with_compiler(mut self, compiler: Compiler) -> Self {
        self.compiler = Some(compiler);
        self
    }

    pub fn with_cpu(mut self, cpu: Cpu) -> Self {
        self.cpu = Some(cpu);
        self
    }

    pub fn with_os(mut self, os: Os) -> Self {
        self.runtime_os = Some(os);
        self
    }

    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }

    pub fn add_include_dir(&mut self, address: Address) {
        self.include_dirs.push(IncludeDirectory { address });
    }
}
