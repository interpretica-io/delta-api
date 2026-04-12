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

/// Type of an ASP address
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum AddressType {
    #[default]
    Unspecified,
    Inet,
    Inet6,
    Nng,
    LocalPath,
    Filter,
    CompilerPath,
}

/// An ASP address (file path, IP endpoint, NNG URL, …)
#[derive(Serialize, Deserialize, PartialEq, Clone, Debug, Default)]
pub struct Address {
    pub addr_type: AddressType,
    /// String form of the address (path, host, URL, …)
    pub urn: Option<String>,
    /// TCP/UDP port (only for INET / INET6)
    pub port: Option<u16>,
}

impl Address {
    /// Create a local-file address
    pub fn local_file(path: impl Into<String>) -> Self {
        Address {
            addr_type: AddressType::LocalPath,
            urn: Some(path.into()),
            port: None,
        }
    }

    /// Create an IP address (host + port)
    pub fn ip(host: impl Into<String>, port: u16) -> Self {
        Address {
            addr_type: AddressType::Inet,
            urn: Some(host.into()),
            port: Some(port),
        }
    }

    /// Create an NNG endpoint address
    pub fn nng(url: impl Into<String>) -> Self {
        Address {
            addr_type: AddressType::Nng,
            urn: Some(url.into()),
            port: None,
        }
    }

    /// Create a filter (regex) address
    pub fn filter(pattern: impl Into<String>) -> Self {
        Address {
            addr_type: AddressType::Filter,
            urn: Some(pattern.into()),
            port: None,
        }
    }

    /// Create a compiler-path address
    pub fn compiler_path(path: impl Into<String>) -> Self {
        Address {
            addr_type: AddressType::CompilerPath,
            urn: Some(path.into()),
            port: None,
        }
    }

    /// Return the string representation used in JSON ("urn" field)
    pub fn to_str(&self) -> Option<&str> {
        self.urn.as_deref()
    }
}