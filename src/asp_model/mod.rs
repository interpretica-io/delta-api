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

// Pure Rust data models for asp requests/results. They carry no FFI and link
// nothing native, so they stay available regardless of the `asp_client` feature.
pub mod address;
pub mod analysis;
pub mod enums;
pub mod env;
pub mod file;
pub mod identification;
pub mod report;
pub mod result;
pub mod status;
pub mod symbol;
pub mod workspace;

// The native libasp client (FFI bindings in `sys`, safe wrapper in
// `connection`) is the only part that links the native library. Gate it behind
// `asp_client` so the crate can be built without a libasp toolchain (e.g. for
// wasm). The build script skips the native build under the same condition.
#[cfg(feature = "asp_client")]
pub(crate) mod sys;

#[cfg(feature = "asp_client")]
pub mod connection;
