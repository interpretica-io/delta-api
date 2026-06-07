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

//! Delta API network server.
//!
//! The server wraps a single shared [`NodePool`] and exposes its operations
//! over TCP using a newline-delimited JSON protocol: each request is one JSON
//! object terminated by `\n`, and each response is one JSON object terminated
//! by `\n`. A connection may carry any number of requests, processed strictly
//! in the order received; a malformed line yields an error response but does
//! not drop the connection.
//!
//! `NodePool` operations are blocking — every [`SshSession`] drives its own
//! Tokio runtime — so each request is executed on a blocking thread via
//! [`tokio::task::spawn_blocking`]. That keeps the async accept loop free and
//! avoids nesting one Tokio runtime inside another.
//!
//! [`SshSession`]: crate::obj_model::ssh_session::SshSession

use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use crate::data_model::conn_alive_status::ConnAliveStatus;
use crate::data_model::conn_status::ConnStatus;
use crate::data_model::deploy_subject::DeploySubject;
use crate::data_model::result::add_result::AddResult;
use crate::data_model::result::connect_result::ConnectResult;
use crate::data_model::result::deploy_result::DeployResult;
use crate::data_model::result::disconnect_result::DisconnectResult;
use crate::data_model::result::remove_result::RemoveResult;
use crate::data_model::result::run_result::RunResult;
use crate::obj_model::node_pool::NodePool;

/// The default address the server binds to when none is supplied.
pub const DEFAULT_BIND_ADDR: &str = "127.0.0.1:7700";

/// A single client request.
///
/// The wire form is a JSON object tagged by an `op` field, for example
/// `{"op":"add","name":"n1","fqdn":"host.example"}`.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    /// Register a node in the pool.
    Add {
        name: String,
        fqdn: String,
        /// Node parameters keyed by their
        /// [`NodeParameters`](crate::data_model::node_parameters::NodeParameters)
        /// name (`Username`, `Password`, `Distr`, `BindAddr`, `BindPort`).
        #[serde(default)]
        params: HashMap<String, String>,
    },
    /// Open an SSH connection to a previously registered node.
    Connect { name: String },
    /// Close the SSH connection to a node.
    Disconnect { name: String },
    /// Forget a node, dropping any open connection to it.
    Remove { name: String },
    /// Deploy a subject onto a connected node.
    Deploy {
        name: String,
        subject: DeploySubject,
    },
    /// Start a deployed subject on a node.
    Run {
        name: String,
        subject: DeploySubject,
    },
    /// Report the connection status of a node.
    IsConnected { name: String },
    /// Probe whether a node's subject process is alive.
    IsAlive { name: String },
    /// List the names of every registered node.
    ListNodes,
    /// Liveness check for the server itself.
    Ping,
}

/// A single server response.
///
/// Adjacently tagged (`op` plus `result`) so the payload may take any shape,
/// e.g. `{"op":"add","result":"Ok"}`, `{"op":"pong"}`, or
/// `{"op":"error","result":{"message":"..."}}`.
#[derive(Debug, Serialize)]
#[serde(tag = "op", content = "result", rename_all = "snake_case")]
pub enum Response {
    Add(AddResult),
    Connect(ConnectResult),
    Disconnect(DisconnectResult),
    Remove(RemoveResult),
    Deploy(DeployResult),
    Run(RunResult),
    IsConnected(ConnStatus),
    IsAlive(ConnAliveStatus),
    ListNodes(Vec<String>),
    /// Successful reply to [`Request::Ping`].
    Pong,
    /// The request line could not be parsed or executed.
    Error {
        message: String,
    },
}

/// Apply one request against the shared pool and produce a response.
///
/// This is synchronous and may block on SSH I/O, so callers should run it off
/// the async runtime — see [`handle_connection`].
pub fn handle_request(pool: &Mutex<NodePool>, req: Request) -> Response {
    // A poisoned mutex means an earlier handler panicked mid-update. Recover
    // the guard instead of propagating the panic to every later request.
    let mut pool = pool.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

    match req {
        Request::Add { name, fqdn, params } => Response::Add(pool.add(name, fqdn, params)),
        Request::Connect { name } => Response::Connect(pool.connect(name)),
        Request::Disconnect { name } => Response::Disconnect(pool.disconnect(name)),
        Request::Remove { name } => Response::Remove(pool.remove(name)),
        Request::Deploy { name, subject } => Response::Deploy(pool.deploy(name, subject)),
        Request::Run { name, subject } => Response::Run(pool.run(name, subject)),
        Request::IsConnected { name } => Response::IsConnected(pool.is_connected(name)),
        Request::IsAlive { name } => Response::IsAlive(pool.is_alive(name)),
        Request::ListNodes => {
            let mut names: Vec<String> = pool.nodes.keys().cloned().collect();
            names.sort();
            Response::ListNodes(names)
        }
        Request::Ping => Response::Pong,
    }
}

/// The Delta API server: a bound TCP listener plus the shared node pool.
pub struct Server {
    listener: TcpListener,
    pool: Arc<Mutex<NodePool>>,
}

impl Server {
    /// Bind the server to `addr` (e.g. `127.0.0.1:7700`). Pass port `0` to let
    /// the OS choose a free port, then read it back with [`Server::local_addr`].
    pub async fn bind(addr: &str) -> io::Result<Server> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Server {
            listener,
            pool: Arc::new(Mutex::new(NodePool::new())),
        })
    }

    /// The address the server is actually listening on.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accept connections forever, serving each on its own task. Returns only
    /// if accepting a connection fails.
    pub async fn run(self) -> io::Result<()> {
        loop {
            let (stream, peer) = self.listener.accept().await?;
            let pool = self.pool.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(stream, pool).await {
                    log::error!("connection {peer} ended with error: {e}");
                }
            });
        }
    }
}

/// Serve a single client connection: read request lines, write response lines.
async fn handle_connection(stream: TcpStream, pool: Arc<Mutex<NodePool>>) -> io::Result<()> {
    let peer = stream.peer_addr().ok();
    let (read_half, mut write_half) = stream.into_split();
    let mut lines = BufReader::new(read_half).lines();

    while let Some(line) = lines.next_line().await? {
        // Blank lines (e.g. keep-alives) carry no request; skip them.
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Request>(&line) {
            Ok(req) => {
                let pool = pool.clone();
                // NodePool operations block on SSH I/O; keep them off the
                // async runtime worker thread.
                match tokio::task::spawn_blocking(move || handle_request(&pool, req)).await {
                    Ok(resp) => resp,
                    Err(join_err) => Response::Error {
                        message: format!("internal handler error: {join_err}"),
                    },
                }
            }
            Err(parse_err) => Response::Error {
                message: format!("invalid request: {parse_err}"),
            },
        };

        // A well-formed `Response` always serializes; fall back to a static
        // error line just in case, so the stream stays line-synchronized.
        let mut encoded = serde_json::to_string(&response).unwrap_or_else(|_| {
            String::from(r#"{"op":"error","result":{"message":"response encoding failed"}}"#)
        });
        encoded.push('\n');

        if let Err(e) = write_half.write_all(encoded.as_bytes()).await {
            log::error!("write to {peer:?} failed: {e}");
            break;
        }
    }

    Ok(())
}
