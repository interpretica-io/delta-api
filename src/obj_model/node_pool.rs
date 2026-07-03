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
                    let runs =
                        Self::sess_exec(sess, &format!("kill -0 {} && echo runs", pid.trim()));
                    if runs.contains("runs") {
                        let bind_addr = Self::sess_exec(sess, "cat /tmp/visao/bind_addr");
                        let bind_port = Self::sess_exec(sess, "cat /tmp/visao/bind_port");
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

    /// Per-subject on-node layout: where the archive lands, where it extracts,
    /// and the binary to invoke. Keeps deploy/run uniform across agents.
    fn subject_layout(subject: &DeploySubject) -> (&'static str, &'static str, &'static str) {
        match subject {
            // (working dir, uploaded archive path, binary)
            DeploySubject::Sa => ("/tmp/visao", "/tmp/visao-archive.tar.xz", "/tmp/visao/bin/visao"),
            DeploySubject::Delta => ("/tmp/delta", "/tmp/delta-archive.tar.xz", "/tmp/delta/bin/delta"),
        }
    }

    pub fn deploy(&mut self, name: String, subject: DeploySubject) -> DeployResult {
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
        let (dir, archive, binary) = Self::subject_layout(&subject);

        if !sess.upload_file(&distr, archive).unwrap_or(false) {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployCopyFailed;
        }
        subject_st.deploy_archive_copied = true;

        let extract = Self::sess_exec(
            sess,
            &format!(
                "mkdir -p {dir} && tar xf {archive} -C {dir} > /dev/null 2> /dev/null && echo ok"
            ),
        );
        if extract.trim().is_empty() {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployExtractionFailed;
        }
        subject_st.deploy_archive_extracted = true;

        let version = Self::sess_exec(sess, &format!("{binary} --version"));
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

        let (dir, _archive, binary) = Self::subject_layout(&subject);

        // Kill existing instance, if any.
        let _ = Self::sess_exec(
            sess,
            &format!(
                "/bin/bash -c 'test -f {dir}/pid && test $(cat {dir}/pid) -gt 0 && kill $(cat {dir}/pid)'"
            ),
        );

        let launch = match subject {
            DeploySubject::Sa => {
                let (bind_addr, bind_port) = self.infer_conn_params(&node);
                vec![
                    format!(
                        "{binary} --server 'tcp://{}:{}' < /dev/null > /dev/null 2> /dev/null &",
                        bind_addr, bind_port
                    ),
                    format!("echo $! > {dir}/pid"),
                    format!("echo {} > {dir}/bind_addr", bind_addr),
                    format!("echo {} > {dir}/bind_port", bind_port),
                ]
            }
            // Delta calls home to a collector rather than binding a port; the
            // control-plane operating model. Flags come from node parameters
            // and are only added when set (a bare node still runs, unsigned).
            DeploySubject::Delta => {
                vec![
                    format!(
                        "{binary} {} < /dev/null > /dev/null 2> /dev/null &",
                        self.delta_run_args(&node)
                    ),
                    format!("echo $! > {dir}/pid"),
                ]
            }
        };

        let mut commands = launch;
        commands.push("sleep 4".to_string());
        commands.push(format!(
            "kill -0 \"$(cat {dir}/pid)\" && echo pid \"$(cat {dir}/pid)\""
        ));

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

    /// A parameter value is safe to embed in a single-quoted shell argument
    /// only if it carries no single quote or control/meta character. Anything
    /// else is dropped (empty), exactly as bind params are, so a hostile or
    /// malformed value can never break out of its quotes.
    fn shell_safe(v: String) -> String {
        if v.is_empty() {
            return v;
        }
        let bad = v
            .chars()
            .any(|c| c.is_control() || matches!(c, '\'' | '"' | '`' | '$' | '\\' | ';' | '&' | '|' | '<' | '>' | '(' | ')' | ' '));
        if bad {
            error!("Dropping delta run parameter with unsafe characters: {}", v);
            String::new()
        } else {
            v
        }
    }

    /// Build the delta agent's call-home argument string from node parameters.
    /// Only sets a flag when its parameter is present and shell-safe.
    fn delta_run_args(&self, node: &Node) -> String {
        let p = |np| Self::shell_safe(self.get_node_param(node, np));

        let interval = {
            let iv = self.get_node_param(node, NodeParameters::Interval);
            match iv.trim().parse::<u32>() {
                Ok(n) if n > 0 => n,
                _ => 60, // sane default: report every minute
            }
        };

        let mut args = vec![format!("--interval {interval}")];

        let collector = p(NodeParameters::CollectorUrl);
        if !collector.is_empty() {
            args.push(format!("--server '{collector}'"));
        }
        let token = p(NodeParameters::Token);
        if !token.is_empty() {
            args.push(format!("--token '{token}'"));
        }
        let verify_key = p(NodeParameters::VerifyKey);
        if !verify_key.is_empty() {
            args.push(format!("--verify-key '{verify_key}'"));
        }
        let stun = p(NodeParameters::Stun);
        if !stun.is_empty() {
            args.push(format!("--stun '{stun}'"));
        }
        let binaries = p(NodeParameters::BinariesDir);
        if !binaries.is_empty() {
            args.push(format!("--binaries '{binaries}'"));
        }
        // --allow-response requires a verify key on the agent side too; only
        // offer it when one is configured, so we never ask for destructive
        // response without the signing that gates it.
        let allow = self.get_node_param(node, NodeParameters::AllowResponse);
        let allow = matches!(allow.trim(), "1" | "true" | "yes" | "on");
        if allow && !verify_key.is_empty() {
            args.push("--allow-response".to_string());
        } else if allow {
            error!("Ignoring AllowResponse: no VerifyKey configured for the node");
        }

        args.join(" ")
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_model::node_parameters::NodeParameters;

    fn node_with(params: &[(NodeParameters, &str)]) -> (NodePool, Node) {
        let mut pool = NodePool::new();
        let mut m = HashMap::new();
        for (k, v) in params {
            m.insert(k.to_string(), v.to_string());
        }
        pool.add("n".to_string(), "host".to_string(), m);
        let node = pool.nodes["n"].clone();
        (pool, node)
    }

    #[test]
    fn delta_args_bare_node_defaults_interval() {
        let (pool, node) = node_with(&[]);
        assert_eq!(pool.delta_run_args(&node), "--interval 60");
    }

    #[test]
    fn delta_args_full_node() {
        let (pool, node) = node_with(&[
            (NodeParameters::Interval, "30"),
            (NodeParameters::CollectorUrl, "https://c.example/ingest"),
            (NodeParameters::Token, "abc123"),
            (
                NodeParameters::VerifyKey,
                "79b5562e8fe654f94078b112e8a98ba7901f853ae695bed7e0e3910bad049664",
            ),
            (NodeParameters::Stun, "stun.l.google.com:19302"),
            (NodeParameters::AllowResponse, "true"),
            (NodeParameters::BinariesDir, "/"),
        ]);
        let args = pool.delta_run_args(&node);
        assert!(args.contains("--interval 30"), "{args}");
        assert!(args.contains("--server 'https://c.example/ingest'"), "{args}");
        assert!(args.contains("--token 'abc123'"), "{args}");
        assert!(args.contains("--verify-key '79b5562e"), "{args}");
        assert!(args.contains("--stun 'stun.l.google.com:19302'"), "{args}");
        assert!(args.contains("--allow-response"), "{args}");
        assert!(args.contains("--binaries '/'"), "{args}");
    }

    #[test]
    fn allow_response_requires_verify_key() {
        // Opt-in set but no key: the destructive flag must not be offered.
        let (pool, node) = node_with(&[(NodeParameters::AllowResponse, "1")]);
        let args = pool.delta_run_args(&node);
        assert!(!args.contains("--allow-response"), "{args}");
    }

    #[test]
    fn drops_shell_injection_in_params() {
        // A collector URL trying to break out of its single quotes is dropped,
        // so --server is simply absent rather than injected.
        let (pool, node) = node_with(&[(
            NodeParameters::CollectorUrl,
            "https://x/'; rm -rf / #",
        )]);
        let args = pool.delta_run_args(&node);
        assert!(!args.contains("--server"), "{args}");
        assert!(!args.contains("rm -rf"), "{args}");
    }

    #[test]
    fn bad_interval_falls_back_to_default() {
        let (pool, node) = node_with(&[(NodeParameters::Interval, "not-a-number")]);
        assert_eq!(pool.delta_run_args(&node), "--interval 60");
        let (pool, node) = node_with(&[(NodeParameters::Interval, "0")]);
        assert_eq!(pool.delta_run_args(&node), "--interval 60");
    }
}
