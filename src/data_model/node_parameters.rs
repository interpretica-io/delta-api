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

#[allow(non_camel_case_types)]
#[derive(strum_macros::Display)]
pub enum NodeParameters {
    Username,
    Password,
    Distr,
    BindAddr,
    BindPort,

    // Delta agent (call-home) run parameters. All optional; a flag is passed
    // only when its parameter is non-empty, so a bare node still runs.
    /// Collector URL the delta agent reports to (`--server`), e.g.
    /// `https://collector.example.com/ingest`.
    CollectorUrl,
    /// Identification token embedded in reports and sent as a bearer
    /// (`--token`).
    Token,
    /// Ed25519 public key (hex) authenticating control commands
    /// (`--verify-key`).
    VerifyKey,
    /// Monitor interval in seconds (`--interval`); defaults to 60 when unset.
    Interval,
    /// "1"/"true" to honor destructive response commands (`--allow-response`).
    AllowResponse,
    /// STUN server for the reachability/​wake beacon (`--stun`).
    Stun,
    /// Directory the agent scans for ELF binary composition/provenance
    /// (`--binaries <dir>`); e.g. `/` to profile the whole rootfs.
    BinariesDir,
}
