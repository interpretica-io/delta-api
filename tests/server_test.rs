/*
 * Delta API — server integration tests.
 *
 * These tests drive the real TCP server end to end. They exercise the
 * operations that do not require a live SSH peer: node registration, lookups,
 * shared state and the error paths. The SSH-dependent operations (connect,
 * deploy, run) are covered here only for their pre-SSH validation behaviour.
 */
#![cfg(feature = "server")]

use delta_api::server::Server;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;

/// A newline-delimited JSON client over a single connection.
struct Client {
    write: OwnedWriteHalf,
    lines: Lines<BufReader<OwnedReadHalf>>,
}

impl Client {
    async fn connect(addr: SocketAddr) -> Client {
        let (read_half, write_half) = TcpStream::connect(addr).await.unwrap().into_split();
        Client {
            write: write_half,
            lines: BufReader::new(read_half).lines(),
        }
    }

    /// Send one request line and return the parsed response object.
    async fn call(&mut self, request: &str) -> Value {
        self.write.write_all(request.as_bytes()).await.unwrap();
        self.write.write_all(b"\n").await.unwrap();
        let line = self
            .lines
            .next_line()
            .await
            .unwrap()
            .expect("server closed the connection");
        serde_json::from_str(&line).unwrap()
    }
}

/// Spawn a server on an OS-chosen port and return its address.
async fn start_server() -> SocketAddr {
    let server = Server::bind("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr().unwrap();
    tokio::spawn(server.run());
    addr
}

#[tokio::test]
async fn lifecycle_and_lookups() {
    let addr = start_server().await;
    let mut c = Client::connect(addr).await;

    // The server answers a liveness probe.
    assert_eq!(c.call(r#"{"op":"ping"}"#).await, json!({"op": "pong"}));

    // A fresh pool has no nodes.
    assert_eq!(
        c.call(r#"{"op":"list_nodes"}"#).await,
        json!({"op": "list_nodes", "result": []})
    );

    // Registering a node succeeds.
    assert_eq!(
        c.call(r#"{"op":"add","name":"n1","fqdn":"host.example","params":{"Username":"u"}}"#)
            .await,
        json!({"op": "add", "result": "Ok"})
    );

    // Registering the same name again is rejected.
    assert_eq!(
        c.call(r#"{"op":"add","name":"n1","fqdn":"host.example"}"#)
            .await,
        json!({"op": "add", "result": "NodeAlreadyExists"})
    );

    // A second node is accepted and the listing is sorted.
    assert_eq!(
        c.call(r#"{"op":"add","name":"a0","fqdn":"other.example"}"#)
            .await,
        json!({"op": "add", "result": "Ok"})
    );
    assert_eq!(
        c.call(r#"{"op":"list_nodes"}"#).await,
        json!({"op": "list_nodes", "result": ["a0", "n1"]})
    );

    // A freshly registered node is not connected.
    let status = c.call(r#"{"op":"is_connected","name":"n1"}"#).await;
    assert_eq!(status["op"], "is_connected");
    assert_eq!(status["result"]["connected"], false);

    // The liveness probe reports a Sa subject entry, alive=false.
    let alive = c.call(r#"{"op":"is_alive","name":"n1"}"#).await;
    assert_eq!(alive["op"], "is_alive");
    assert_eq!(alive["result"]["subjects"]["Sa"]["alive"], false);

    // Removing a node succeeds once; the second attempt fails.
    assert_eq!(
        c.call(r#"{"op":"remove","name":"n1"}"#).await,
        json!({"op": "remove", "result": "Ok"})
    );
    assert_eq!(
        c.call(r#"{"op":"remove","name":"n1"}"#).await,
        json!({"op": "remove", "result": "NodeNotFound"})
    );
}

#[tokio::test]
async fn unknown_node_operations_report_not_found() {
    let addr = start_server().await;
    let mut c = Client::connect(addr).await;

    assert_eq!(
        c.call(r#"{"op":"connect","name":"ghost"}"#).await,
        json!({"op": "connect", "result": "NodeNotFound"})
    );
    assert_eq!(
        c.call(r#"{"op":"disconnect","name":"ghost"}"#).await,
        json!({"op": "disconnect", "result": "NodeNotFound"})
    );
    assert_eq!(
        c.call(r#"{"op":"run","name":"ghost","subject":"Sa"}"#)
            .await,
        json!({"op": "run", "result": "NodeNotFound"})
    );

    // Deploying the Delta subject is rejected before any node lookup.
    assert_eq!(
        c.call(r#"{"op":"deploy","name":"ghost","subject":"Delta"}"#)
            .await,
        json!({"op": "deploy", "result": "InvalidArgument"})
    );
    // Deploying a Sa subject onto an unknown node reports NodeNotFound.
    assert_eq!(
        c.call(r#"{"op":"deploy","name":"ghost","subject":"Sa"}"#)
            .await,
        json!({"op": "deploy", "result": "NodeNotFound"})
    );
}

#[tokio::test]
async fn deploy_and_run_require_a_connection() {
    let addr = start_server().await;
    let mut c = Client::connect(addr).await;

    assert_eq!(
        c.call(r#"{"op":"add","name":"n1","fqdn":"host.example"}"#)
            .await,
        json!({"op": "add", "result": "Ok"})
    );

    // The node exists but was never connected.
    assert_eq!(
        c.call(r#"{"op":"deploy","name":"n1","subject":"Sa"}"#)
            .await,
        json!({"op": "deploy", "result": "NodeNotConnected"})
    );
    assert_eq!(
        c.call(r#"{"op":"run","name":"n1","subject":"Sa"}"#).await,
        json!({"op": "run", "result": "NodeNotConnected"})
    );
}

#[tokio::test]
async fn malformed_requests_produce_errors_without_dropping_the_connection() {
    let addr = start_server().await;
    let mut c = Client::connect(addr).await;

    // Not JSON at all.
    let resp = c.call("this is not json").await;
    assert_eq!(resp["op"], "error");
    assert!(resp["result"]["message"].is_string());

    // Valid JSON, unknown operation.
    let resp = c.call(r#"{"op":"teleport","name":"n1"}"#).await;
    assert_eq!(resp["op"], "error");

    // Valid JSON, missing a required field.
    let resp = c.call(r#"{"op":"add","name":"n1"}"#).await;
    assert_eq!(resp["op"], "error");

    // The connection survives every error above and still serves requests.
    assert_eq!(c.call(r#"{"op":"ping"}"#).await, json!({"op": "pong"}));
}

#[tokio::test]
async fn pool_state_is_shared_across_connections() {
    let addr = start_server().await;

    let mut writer = Client::connect(addr).await;
    assert_eq!(
        writer
            .call(r#"{"op":"add","name":"shared","fqdn":"h"}"#)
            .await,
        json!({"op": "add", "result": "Ok"})
    );

    // A second, independent connection sees the node added by the first.
    let mut reader = Client::connect(addr).await;
    assert_eq!(
        reader.call(r#"{"op":"list_nodes"}"#).await,
        json!({"op": "list_nodes", "result": ["shared"]})
    );
}

#[tokio::test]
async fn multiple_requests_are_pipelined_on_one_connection() {
    let addr = start_server().await;
    let mut c = Client::connect(addr).await;

    // Three requests written back to back, responses returned in order.
    c.write
        .write_all(
            b"{\"op\":\"ping\"}\n\
              {\"op\":\"add\",\"name\":\"p\",\"fqdn\":\"h\"}\n\
              {\"op\":\"list_nodes\"}\n",
        )
        .await
        .unwrap();

    async fn next(lines: &mut Lines<BufReader<OwnedReadHalf>>) -> Value {
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str::<Value>(&line).unwrap()
    }
    assert_eq!(next(&mut c.lines).await, json!({"op": "pong"}));
    assert_eq!(
        next(&mut c.lines).await,
        json!({"op": "add", "result": "Ok"})
    );
    assert_eq!(
        next(&mut c.lines).await,
        json!({"op": "list_nodes", "result": ["p"]})
    );
}
