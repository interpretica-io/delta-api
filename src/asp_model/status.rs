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

use std::fmt;

/// Error codes matching asp_error in C
#[derive(Debug, Clone, PartialEq)]
pub enum AspError {
    FdFailure,
    ConnectionFailed,
    NoMemory,
    Io(String),
    MalformedResult,
    Fail,
    InvalidArgument,
    NoEntry,
    TimedOut,
}

impl fmt::Display for AspError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AspError::FdFailure => write!(f, "file descriptor failure"),
            AspError::ConnectionFailed => write!(f, "connection failed"),
            AspError::NoMemory => write!(f, "not enough memory"),
            AspError::Io(msg) => write!(f, "I/O error: {}", msg),
            AspError::MalformedResult => write!(f, "malformed result"),
            AspError::Fail => write!(f, "generic failure"),
            AspError::InvalidArgument => write!(f, "invalid argument"),
            AspError::NoEntry => write!(f, "no entry found"),
            AspError::TimedOut => write!(f, "connection timed out"),
        }
    }
}

impl std::error::Error for AspError {}

impl From<std::io::Error> for AspError {
    fn from(e: std::io::Error) -> Self {
        AspError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AspError {
    fn from(_e: serde_json::Error) -> Self {
        AspError::MalformedResult
    }
}

pub type AspResult<T> = Result<T, AspError>;
