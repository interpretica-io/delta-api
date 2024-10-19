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

use std::net::TcpStream;
use ssh2::Session;
use std::collections::HashMap;
use log::info;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
#[repr(C)]
pub struct Node {
    pub fqdn: String,
    pub username: String,
    pub password: String,
}

pub struct NodePool {
    pub nodes: HashMap<String, Node>,
    pub sessions: HashMap<String, Session>,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub enum ConnectResult {
    Ok,
    NodeNotFound,
    NotAuthenticated,
}

impl NodePool {
    pub fn add_node(&mut self, name: String, fqdn: String, username: String, password: String)  {
        info!("Adding node {}", fqdn);
        self.nodes.insert(name, Node { fqdn: fqdn, username: username, password: password });
    }

    pub fn connect(&mut self, name: String) -> ConnectResult
    {
        if !self.nodes.contains_key(&name) {
            return ConnectResult::NodeNotFound;
        }

        if self.sessions.contains_key(&name) {
            self.sessions.remove(&name);
        }

        let node = &self.nodes[&name];
        let tcp = TcpStream::connect(node.fqdn.clone()).unwrap();
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().unwrap();
        sess.userauth_password(&node.username.clone(), &node.password.clone()).unwrap();
        if !sess.authenticated() {
            return ConnectResult::NotAuthenticated;
        }

        self.sessions.insert(name, sess);
        return ConnectResult::Ok;
    }
}
