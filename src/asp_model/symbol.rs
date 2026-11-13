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

use super::enums::{ResourceType, SymbolOrigin};
use super::file::File;
use serde::{Deserialize, Serialize};

/// A global symbol (variable or function)
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct Symbol {
    pub namespace: Option<String>,
    pub name: Option<String>,
    pub resource_type: ResourceType,
    pub origin: SymbolOrigin,
    /// What the entity is: `function` / `type` / `macro` / `variable`.
    /// `None` when the analyzer did not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Stable identity across runs (mangled or fully-qualified), when the
    /// analyzer provides one. `None` otherwise — compose from namespace+name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qualified_name: Option<String>,
    /// Declaration/signature text, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// File the symbol is defined in, for a symbol that came from source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub def_file: Option<String>,
    /// Line it is defined on; `0` when unknown.
    #[serde(default)]
    pub def_line: u32,
}

/// Identity excludes the definition site, the signature and the kind — where a
/// symbol sits and how it is spelled does not change which symbol it is. This
/// mirrors libasp, where the hash folds in only name, namespace and the
/// resource flags, and keeps dedup in `symbols.rs` behaving as before.
impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        self.namespace == other.namespace
            && self.name == other.name
            && self.resource_type == other.resource_type
            && self.origin == other.origin
    }
}

/// A location group containing files and symbols
/// (per-file or connected-area grouping)
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct SymbolLocation {
    pub name: Option<String>,
    pub files: Vec<File>,
    pub symbols: Vec<Symbol>,
}

/// A caller→callee pair from the call map
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct SymbolCall {
    pub callers: Vec<Symbol>,
    pub callees: Vec<Symbol>,
}
