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

use crate::data_model::conn_alive_status::*;
use crate::data_model::conn_status::ConnStatus;
use crate::data_model::deploy_subject::DeploySubject;
use crate::data_model::instance::Instance;
use crate::data_model::node_parameters::NodeParameters;
use crate::data_model::result::add_result::AddResult;
use crate::data_model::result::connect_result::ConnectResult;
use crate::data_model::result::deploy_result::DeployResult;
use crate::data_model::result::disconnect_result::DisconnectResult;
use crate::data_model::result::remove_result::RemoveResult;
use crate::data_model::result::run_result::RunResult;
use crate::obj_model::node::Node;
use crate::obj_model::ssh_session::SshSession;
use log::{error, info};
use std::collections::HashMap;

pub struct NodePool {
    pub nodes: HashMap<String, Node>,
    pub instances: HashMap<String, Instance>,
    pub str_params: HashMap<String, String>,
}

impl NodePool {
    pub fn new() -> NodePool {
        NodePool {
            nodes: HashMap::new(),
            instances: HashMap::new(),
            str_params: HashMap::new(),
        }
    }

    pub fn get_node_param(&self, node: &Node, param: NodeParameters) -> String {
        let sparam = param.to_string();
        if let Some(v) = node.str_params.get(&sparam) {
            return v.clone();
        }
        if let Some(v) = self.str_params.get(&sparam) {
            return v.clone();
        }
        String::new()
    }

    pub fn add(
        &mut self,
        name: String,
        fqdn: String,
        node_params: HashMap<String, String>,
    ) -> AddResult {
        if self.nodes.contains_key(&name) {
            error!("Node already exists: {}", name);
            return AddResult::NodeAlreadyExists;
        }

        self.nodes.insert(
            name,
            Node {
                fqdn: fqdn.clone(),
                str_params: node_params,
            },
        );

        info!("Added node {}", fqdn);
        AddResult::Ok
    }

    pub fn is_connected(&self, name: String) -> ConnStatus {
        if let Some(inst) = self.instances.get(&name) {
            return inst.conn_status.clone();
        }
        ConnStatus::new(false)
    }

    pub fn is_alive(&self, name: String) -> ConnAliveStatus {
        let mut conn_alive_status = ConnAliveStatus::new();
        let mut subj_alive_status = SubjectAliveStatus::new();

        if let Some(inst) = self.instances.get(&name) {
            if let Some(sess) = inst.ssh_session.as_ref() {
                let pid = Self::sess_exec(sess, "cat /tmp/visao/pid");
                if pid.trim().parse::<u64>().is_ok() {
                    let runs = Self::sess_exec(
                        sess,
                        &format!("kill -0 {} && echo runs", pid.trim()),
                    );
                    if runs.contains("runs") {
                        let bind_addr =
                            Self::sess_exec(sess, "cat /tmp/visao/bind_addr");
                        let bind_port =
                            Self::sess_exec(sess, "cat /tmp/visao/bind_port");
                        if let Ok(port) = bind_port.trim().parse::<u16>() {
                            subj_alive_status.alive = true;
                            subj_alive_status.bind_addr = bind_addr.trim().to_string();
                            subj_alive_status.bind_port = port;
                        }
                    }
                }
            }
        }

        conn_alive_status
            .subjects
            .insert(DeploySubject::Sa, subj_alive_status);
        conn_alive_status
    }

    pub fn connect(&mut self, name: String) -> ConnectResult {
        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return ConnectResult::NodeNotFound;
        }

        if self.instances.contains_key(&name) {
            self.instances.remove(&name);
        }

        let node = &self.nodes[&name];
        let user = self.get_node_param(node, NodeParameters::Username);
        let password = self.get_node_param(node, NodeParameters::Password);

        // Default port 22 unless an explicit `host:port` was supplied as fqdn.
        let addr = if node.fqdn.contains(':') {
            node.fqdn.clone()
        } else {
            format!("{}:22", node.fqdn)
        };

        let sess = match SshSession::connect(&addr, &user, &password) {
            Ok(s) => s,
            Err(e) => {
                error!("SSH connect failed for {}: {}", name, e);
                return ConnectResult::NotAuthenticated;
            }
        };

        let plat = Self::sess_exec(&sess, "uname -a");
        let mut inst = Instance::new_ssh(sess, true);
        inst.conn_status.platform = plat;
        self.instances.insert(name.clone(), inst);

        info!("Connected node: {}", name);
        ConnectResult::Ok
    }

    pub fn disconnect(&mut self, name: String) -> DisconnectResult {
        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return DisconnectResult::NodeNotFound;
        }

        if let Some(inst) = self.instances.remove(&name) {
            if let Some(sess) = inst.ssh_session.as_ref() {
                sess.disconnect();
            }
        }

        info!("Disconnected node: {}", name);
        DisconnectResult::Ok
    }

    pub fn remove(&mut self, name: String) -> RemoveResult {
        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return RemoveResult::NodeNotFound;
        }

        self.nodes.remove(&name);
        if let Some(inst) = self.instances.remove(&name) {
            if let Some(sess) = inst.ssh_session.as_ref() {
                sess.disconnect();
            }
        }

        info!("Removed node: {}", name);
        RemoveResult::Ok
    }

    pub fn deploy(&mut self, name: String, subject: DeploySubject) -> DeployResult {
        if subject == DeploySubject::Delta {
            return DeployResult::InvalidArgument;
        }

        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return DeployResult::NodeNotFound;
        }

        if !self.instances.contains_key(&name) {
            error!("Node not connected: {}", name);
            return DeployResult::NodeNotConnected;
        }

        let node = self.nodes[&name].clone();
        let inst = &self.instances[&name];

        let mut conn_status = inst.conn_status.clone();
        let mut subject_st = conn_status.get_subject(subject.clone());

        subject_st.deployed = false;
        subject_st.deploy_archive_copied = false;
        subject_st.deploy_archive_extracted = false;
        subject_st.deploy_archive_tested = false;

        let sess = inst.ssh_session.as_ref().unwrap();
        let distr = self.get_node_param(&node, NodeParameters::Distr);

        if !sess
            .upload_file(&distr, "/tmp/visao-archive.tar.xz")
            .unwrap_or(false)
        {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployCopyFailed;
        }
        subject_st.deploy_archive_copied = true;

        let extract = Self::sess_exec(
            sess,
            "tar xvf /tmp/visao-archive.tar.xz -C /tmp/visao > /dev/null 2> /dev/null && echo ok",
        );
        if extract.trim().is_empty() {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployExtractionFailed;
        }
        subject_st.deploy_archive_extracted = true;

        let version = Self::sess_exec(sess, "/tmp/visao/bin/visao --version");
        if version.trim().is_empty() {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployTestFailed;
        }
        subject_st.deploy_archive_tested = true;

        subject_st.deployed = true;
        conn_status.set_subject(subject, subject_st);
        self.set_state(name, conn_status);
        DeployResult::Ok
    }

    pub fn run(&mut self, name: String, subject: DeploySubject) -> RunResult {
        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return RunResult::NodeNotFound;
        }

        if !self.instances.contains_key(&name) {
            error!("Node not connected: {}", name);
            return RunResult::NodeNotConnected;
        }

        let node = self.nodes[&name].clone();
        let inst = &self.instances[&name];
        let sess = inst.ssh_session.as_ref().unwrap();

        let mut conn_status = inst.conn_status.clone();
        let mut subject_st = conn_status.get_subject(subject.clone());
        subject_st.running = false;

        // Kill existing instance, if any.
        let _ = Self::sess_exec(
            sess,
            "/bin/bash -c 'test -f /tmp/visao/pid && test $(cat /tmp/visao/pid) -gt 0 && kill $(cat /tmp/visao/pid)'",
        );

        let (bind_addr, bind_port) = self.infer_conn_params(&node);
        let commands = vec![
            format!(
                "/tmp/visao/bin/visao --server 'tcp://{}:{}' < /dev/null > /dev/null 2> /dev/null &",
                bind_addr, bind_port
            ),
            "echo $! > /tmp/visao/pid".to_string(),
            format!("echo {} > /tmp/visao/bind_addr", bind_addr),
            format!("echo {} > /tmp/visao/bind_port", bind_port),
            "sleep 4".to_string(),
            "kill -0 \"$(cat /tmp/visao/pid)\" && echo pid \"$(cat /tmp/visao/pid)\"".to_string(),
        ];

        let exec_result = sess.shell_exec(&commands).unwrap_or_default();
        if !exec_result.contains("pid") {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return RunResult::RunFailed;
        }

        subject_st.running = true;
        conn_status.set_subject(subject, subject_st);
        self.set_state(name, conn_status);
        RunResult::Ok
    }

    fn sess_exec(sess: &SshSession, cmd: &str) -> String {
        match sess.exec(cmd) {
            Ok(r) => r.stdout,
            Err(e) => {
                error!("SSH exec failed ({}): {}", cmd, e);
                String::new()
            }
        }
    }

    fn set_state(&mut self, name: String, conn_status: ConnStatus) {
        if let Some(inst) = self.instances.get_mut(&name) {
            inst.conn_status = conn_status;
        }
    }

    fn infer_conn_params(&self, node: &Node) -> (String, String) {
        let mut bind_addr = self.get_node_param(node, NodeParameters::BindAddr);
        if bind_addr.contains('\'') || bind_addr.contains('"') {
            error!("Reset bind address due to bad symbols: {}", bind_addr);
            bind_addr.clear();
        }
        if bind_addr.is_empty() {
            bind_addr = "127.0.0.1".to_string();
        }

        let mut bind_port = self.get_node_param(node, NodeParameters::BindPort);
        if bind_port.parse::<u16>().is_err() {
            error!("Reset bind port due to bad symbols: {}", bind_port);
            bind_port.clear();
        }
        if bind_port.is_empty() {
            bind_port = "5700".to_string();
        }
        (bind_addr, bind_port)
    }
}
