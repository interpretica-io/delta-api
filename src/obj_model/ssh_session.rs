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

//! Pure-Rust SSH client wrapper around `russh`, exposing a blocking API
//! so synchronous callers (NodePool, Instance) don't have to be async.
//!
//! The session owns a small dedicated multi-threaded Tokio runtime; callers
//! may invoke methods from any thread, including from synchronous contexts
//! such as the plugin's periodic hook.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use russh::client::{self, Handle};
use russh::keys::key::PublicKey;
use russh::{ChannelMsg, Disconnect};
use tokio::runtime::Runtime;
use tokio::sync::Mutex;

/// Permissive client handler: ignore host-key validation.
///
/// The original `ssh2`-based implementation didn't verify host keys either
/// (no known_hosts), so behaviour is preserved.
struct AcceptAnyKey;

#[async_trait]
impl client::Handler for AcceptAnyKey {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        _server_public_key: &PublicKey,
    ) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// Result of a remote command execution.
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<u32>,
}

impl ExecResult {
    pub fn success(&self) -> bool {
        matches!(self.exit_code, Some(0))
    }
}

/// SSH session with a synchronous API backed by an internal Tokio runtime.
pub struct SshSession {
    rt: Arc<Runtime>,
    // `Handle` itself is not `Clone`. We share it through `Arc<Mutex<_>>`
    // because some methods (auth) need `&mut`, while channel openers need
    // only `&`. Using a Mutex keeps the API straightforwardly thread-safe.
    handle: Arc<Mutex<Handle<AcceptAnyKey>>>,
}

impl SshSession {
    /// Establish an SSH connection and authenticate with password.
    pub fn connect(addr: &str, user: &str, password: &str) -> io::Result<Self> {
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .thread_name("delta-api-ssh")
                .build()
                .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("tokio runtime: {e}")))?,
        );

        let addr_owned = addr.to_string();
        let user_owned = user.to_string();
        let password_owned = password.to_string();

        let handle = rt
            .block_on(async move {
                let config = Arc::new(client::Config {
                    inactivity_timeout: Some(Duration::from_secs(600)),
                    ..Default::default()
                });
                let mut h = client::connect(config, addr_owned.as_str(), AcceptAnyKey).await?;
                let auth_ok = h.authenticate_password(&user_owned, &password_owned).await?;
                if !auth_ok {
                    return Err(russh::Error::NoAuthMethod);
                }
                Ok::<Handle<AcceptAnyKey>, russh::Error>(h)
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("ssh connect: {e}")))?;

        Ok(SshSession {
            rt,
            handle: Arc::new(Mutex::new(handle)),
        })
    }

    /// Execute a command via `exec` channel request and capture output + exit code.
    pub fn exec(&self, cmd: &str) -> io::Result<ExecResult> {
        let cmd_owned = cmd.to_string();
        let handle = self.handle.clone();
        self.rt
            .block_on(async move {
                let h = handle.lock().await;
                let mut channel = h.channel_open_session().await?;
                channel.exec(true, cmd_owned.as_bytes()).await?;

                let mut stdout: Vec<u8> = Vec::new();
                let mut stderr: Vec<u8> = Vec::new();
                let mut exit_code: Option<u32> = None;

                while let Some(msg) = channel.wait().await {
                    match msg {
                        ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                        ChannelMsg::ExtendedData { ref data, ext } if ext == 1 => {
                            stderr.extend_from_slice(data)
                        }
                        ChannelMsg::ExitStatus { exit_status } => exit_code = Some(exit_status),
                        ChannelMsg::Eof | ChannelMsg::Close => {}
                        _ => {}
                    }
                }
                Ok::<ExecResult, russh::Error>(ExecResult {
                    stdout: String::from_utf8_lossy(&stdout).to_string(),
                    stderr: String::from_utf8_lossy(&stderr).to_string(),
                    exit_code,
                })
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("ssh exec: {e}")))
    }

    /// Open an interactive shell, feed all commands followed by `exit`, and
    /// return concatenated stdout. Mirrors the prior `execute_vec` semantics.
    pub fn shell_exec(&self, commands: &[String]) -> io::Result<String> {
        let commands_owned: Vec<String> = commands.to_vec();
        let handle = self.handle.clone();
        self.rt
            .block_on(async move {
                let h = handle.lock().await;
                let mut channel = h.channel_open_session().await?;
                channel.request_shell(true).await?;

                for cmd in &commands_owned {
                    channel.data(cmd.as_bytes()).await?;
                    channel.data(&b"\n"[..]).await?;
                }
                // Tell the remote shell to exit so we drain output and EOF.
                channel.data(&b"exit\n"[..]).await?;
                channel.eof().await?;

                let mut stdout: Vec<u8> = Vec::new();
                while let Some(msg) = channel.wait().await {
                    match msg {
                        ChannelMsg::Data { ref data } => stdout.extend_from_slice(data),
                        ChannelMsg::Close | ChannelMsg::Eof => {}
                        _ => {}
                    }
                }
                Ok::<String, russh::Error>(String::from_utf8_lossy(&stdout).to_string())
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("ssh shell: {e}")))
    }

    /// Upload a local file to a remote path by piping its contents into
    /// `cat > <remote_path>`. No SCP/SFTP subsystem dependency.
    pub fn upload_file(&self, local_path: &str, remote_path: &str) -> io::Result<bool> {
        let data = std::fs::read(local_path)?;
        let remote_quoted = remote_path.replace('\'', "'\\''");
        let cmd = format!("cat > '{}'", remote_quoted);
        let handle = self.handle.clone();
        self.rt
            .block_on(async move {
                let h = handle.lock().await;
                let mut channel = h.channel_open_session().await?;
                channel.exec(true, cmd.as_bytes()).await?;
                channel.data(&data[..]).await?;
                channel.eof().await?;

                let mut exit_code: Option<u32> = None;
                while let Some(msg) = channel.wait().await {
                    if let ChannelMsg::ExitStatus { exit_status } = msg {
                        exit_code = Some(exit_status);
                    }
                }
                Ok::<bool, russh::Error>(matches!(exit_code, Some(0)))
            })
            .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("ssh upload: {e}")))
    }

    /// Best-effort graceful disconnect. Errors are ignored.
    pub fn disconnect(&self) {
        let handle = self.handle.clone();
        let _ = self.rt.block_on(async move {
            let h = handle.lock().await;
            h.disconnect(Disconnect::ByApplication, "", "en").await
        });
    }
}
