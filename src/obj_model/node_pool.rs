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

use crate::data_model::deploy_subject::DeploySubject;
use crate::data_model::result::run_result::RunResult;
use crate::data_model::instance::Instance;
use crate::data_model::conn_status::ConnStatus;
use crate::data_model::node_parameters::NodeParameters;
use crate::data_model::result::add_result::AddResult;
use crate::data_model::result::connect_result::ConnectResult;
use crate::data_model::result::deploy_result::DeployResult;
use crate::data_model::result::disconnect_result::DisconnectResult;
use crate::data_model::result::remove_result::RemoveResult;
use crate::obj_model::node::Node;
use log::error;
use log::info;
use ssh2::Session;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::io::{BufReader, Read};
use std::net::TcpStream;
use std::path::Path;

pub struct NodePool {
    pub nodes: HashMap<String, Node>,
    pub instances: HashMap<String, Instance>,
    pub str_params: HashMap<String, String>,
}

impl NodePool {
    pub fn new() -> NodePool {
        return NodePool {
            nodes: HashMap::new(),
            instances: HashMap::new(),
            str_params: HashMap::new(),
        };
    }

    pub fn get_node_param(&self, node: &Node, param: NodeParameters) -> String {
        let sparam = param.to_string();
        if node.str_params.contains_key(&sparam) {
            return node.str_params[&sparam].clone();
        }

        if self.str_params.contains_key(&sparam) {
            return self.str_params[&sparam].clone();
        }

        return "".to_string();
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
                str_params: node_params.clone(),
            },
        );

        info!("Added node {}", fqdn);
        return AddResult::Ok;
    }

    pub fn is_connected(&self, name: String) -> ConnStatus {
        if self.instances.contains_key(&name) {
            return self.instances[&name].conn_status.clone();
        }
        return ConnStatus::new(false);
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
        let tcp = TcpStream::connect(node.fqdn.clone()).unwrap();
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().unwrap();
        let auth_result = sess.userauth_password(
            &self.get_node_param(node, NodeParameters::Username),
            &self.get_node_param(node, NodeParameters::Password),
        );
        match auth_result {
            Ok(_r) => {}
            Err(e) => {
                error!("Credentials not accepted: {} (error '{}')", name, e);
                return ConnectResult::NotAuthenticated;
            }
        }

        if !sess.authenticated() {
            error!("Failed to authenticate: {}", name);
            return ConnectResult::NotAuthenticated;
        }

        let plat = self.execute(&sess, "uname -a".to_string());
        let mut inst = Instance::new_ssh(sess, true);
        inst.conn_status.platform = plat;
        self.instances.insert(name.clone(), inst);

        info!("Connected node: {}", name);
        return ConnectResult::Ok;
    }

    pub fn disconnect(&mut self, name: String) -> DisconnectResult {
        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return DisconnectResult::NodeNotFound;
        }

        if self.instances.contains_key(&name) {
            self.instances.remove(&name);
        }

        info!("Disconnected node: {}", name);
        return DisconnectResult::Ok;
    }

    pub fn remove(&mut self, name: String) -> RemoveResult {
        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return RemoveResult::NodeNotFound;
        }

        self.nodes.remove(&name);

        if self.instances.contains_key(&name) {
            self.instances.remove(&name);
        }

        info!("Removed node: {}", name);
        return RemoveResult::Ok;
    }

    pub fn deploy(&mut self, name: String, subject: DeploySubject) -> DeployResult {
        if subject == DeploySubject::Delta {
            return DeployResult::InvalidArgument;
        }

        // Deploy Sa

        if !self.nodes.contains_key(&name) {
            error!("Node doesn't exist: {}", name);
            return DeployResult::NodeNotFound;
        }

        if !self.instances.contains_key(&name) {
            error!("Node not connected: {}", name);
            return DeployResult::NodeNotConnected;
        }

        let node = &self.nodes[&name];
        let inst = &self.instances[&name];

        let mut conn_status = inst.conn_status.clone();
        let mut subject_st = conn_status.get_subject(subject.clone());

        subject_st.deployed = false;
        subject_st.deploy_archive_copied = false;
        subject_st.deploy_archive_extracted = false;
        subject_st.deploy_archive_tested = false;

        if !self.upload_file(
            &inst.ssh_session.as_ref().unwrap(),
            self.get_node_param(node, NodeParameters::Distr),
            "/tmp/visao-archive.tar.xz".to_string(),
        ) {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployCopyFailed;
        }

        subject_st.deploy_archive_copied = true;

        if self.execute(
            &inst.ssh_session.as_ref().unwrap(),
            "tar xvf /tmp/visao-archive.tar.xz -C /tmp/visao".to_string(),
        ) == ""
        {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployExtractionFailed;
        }

        subject_st.deploy_archive_extracted = true;

        if self.execute(
            &inst.ssh_session.as_ref().unwrap(),
            "/tmp/visao/bin/visao --version".to_string(),
        ) == ""
        {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return DeployResult::DeployTestFailed;
        }

        subject_st.deploy_archive_tested = true;

        subject_st.deployed = true;
        conn_status.set_subject(subject, subject_st);
        self.set_state(name, conn_status);
        return DeployResult::Ok;
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

        let inst = &self.instances[&name];

        let mut conn_status = inst.conn_status.clone();
        let mut subject_st = conn_status.get_subject(subject.clone());

        subject_st.running = false;

        if self.execute(
            &inst.ssh_session.as_ref().unwrap(),
            "/bin/bash -c \"kill $(cat /tmp/visao/pid) ; ./build/fs/bin/visao --server tcp://127.0.0.1:5700 & > /dev/null 2> /dev/null ; echo $! > /tmp/visao/pid\"".to_string()
            ) == "" {
            conn_status.set_subject(subject, subject_st);
            self.set_state(name, conn_status);
            return RunResult::RunFailed;
        }

        subject_st.running = true;
        conn_status.set_subject(subject, subject_st);
        self.set_state(name, conn_status);
        return RunResult::Ok;
    }
    fn upload_file(&self, sess: &Session, local_path: String, remote_path: String) -> bool {
        let file = File::open(local_path);
        let file = match file {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open local file: {}", e);
                return false;
            }
        };

        let remote_file = sess.scp_send(Path::new(&remote_path), 0o644, 10, None);

        match remote_file {
            Ok(ref _n) => {}
            Err(_e) => {
                return false;
            }
        }

        let mut remote_file = remote_file.unwrap();
        let mut reader = BufReader::new(file);
        let mut buffer = vec![0; 4096];
        loop {
            let n = reader.read(&mut buffer);
            match n {
                Ok(_n) => {}
                Err(_e) => {
                    break;
                }
            }

            let n = n.unwrap();
            if n == 0 {
                break;
            }

            let _ = remote_file.write_all(&buffer[..n]);
        }

        remote_file.send_eof().unwrap();
        remote_file.wait_eof().unwrap();
        remote_file.close().unwrap();
        remote_file.wait_close().unwrap();
        return true;
    }

    fn execute(&self, sess: &Session, cmd: String) -> String {
        let mut channel = sess.channel_session().unwrap();
        channel.exec(&cmd).unwrap();
        let mut s = String::new();
        channel.read_to_string(&mut s).unwrap();
        let _ = channel.wait_close();

        return s;
    }

    fn set_state(&mut self, name: String, conn_status: ConnStatus)
    {
        let m = self.instances.get_mut(&name);
        m.unwrap().conn_status = conn_status.clone();
    }
}
