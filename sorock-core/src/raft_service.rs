use crate::*;
use anyhow::Result;

use cluster_map::Change;
use lol_core::simple::RaftAppSimple;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(serde::Deserialize, serde::Serialize)]
struct Snapshot {
    table: asura::Table,
    uri_map: HashMap<URI, u64>,
    next_id: u64,
    version: u64,
}
impl Snapshot {
    fn encode(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
    fn decode(b: &[u8]) -> Self {
        bincode::deserialize(b).unwrap()
    }
}

pub struct State {
    cluster: asura::Cluster,
    uri_map: HashMap<URI, u64>,
    next_id: u64,
    version: u64,
    last_change: Change,
}
impl State {
    fn new() -> Self {
        Self {
            cluster: asura::Cluster::new(),
            uri_map: HashMap::new(),
            next_id: 0,
            version: 0,
            last_change: Change::Set,
        }
    }
    fn add_node(&mut self, uri: URI, cap: f64) {
        if !self.uri_map.contains_key(&uri) {
            let node_id = self.next_id;
            self.uri_map.insert(uri.clone(), node_id);
            self.cluster.add_nodes([asura::Node { node_id, cap }]);
            self.next_id += 1;
            self.version += 1;
            self.last_change = Change::Add(uri.0)
        }
    }
    fn remove_node(&mut self, uri: URI) {
        if self.uri_map.contains_key(&uri) {
            let node_id = *self.uri_map.get(&uri).unwrap();
            self.uri_map.remove(&uri);
            self.cluster.remove_node(node_id);
            self.version += 1;
            self.last_change = Change::Remove(uri.0)
        }
    }
    fn make_cluster_map(&self) -> ClusterMap {
        let cluster = asura::Cluster::from_table(self.cluster.dump_table());
        let mut idmap = HashMap::new();
        for (k, v) in &self.uri_map {
            idmap.insert(*v, k.clone());
        }
        ClusterMap::build(self.version, self.last_change.clone(), cluster, idmap)
    }
}

pub struct App {
    pub cluster_in_cli: cluster_in::ClientT,
    pub state: RwLock<State>,
}
impl App {
    pub fn new(cluster_in_cli: cluster_in::ClientT) -> Self {
        Self {
            cluster_in_cli,
            state: RwLock::new(State::new()),
        }
    }
}
#[tonic::async_trait]
impl RaftAppSimple for App {
    async fn process_write(&self, req: &[u8]) -> Result<(Vec<u8>, Option<Vec<u8>>)> {
        let command = Command::decode(&req);
        match command {
            Command::AddNode { uri, cap } => self.state.write().await.add_node(uri, cap),
            Command::RemoveNode { uri } => self.state.write().await.remove_node(uri),
        }

        let cm = self.state.read().await.make_cluster_map();
        // dbg!(cm.members());
        let mut cli = self.cluster_in_cli.clone();
        cli.set_new_cluster(cm).await?;

        let reader = self.state.read().await;
        let table = reader.cluster.dump_table();
        let uri_map = reader.uri_map.clone();
        let next_id = reader.next_id;
        let version = reader.version;
        let snapshot = Snapshot {
            table,
            uri_map,
            next_id,
            version,
        };
        Ok((vec![], Some(Snapshot::encode(&snapshot))))
        // Ok((vec![], None))
    }
    async fn install_snapshot(&self, snapshot: Option<&[u8]>) -> Result<()> {
        let init_state = match snapshot {
            None => State::new(),
            Some(snapshot) => {
                let snapshot = Snapshot::decode(&snapshot);
                let cluster = asura::Cluster::from_table(snapshot.table);
                let next_id = snapshot.next_id;
                let version = snapshot.version;
                State {
                    version,
                    cluster,
                    uri_map: snapshot.uri_map,
                    next_id,
                    last_change: Change::Set,
                }
            }
        };
        let mut writer = self.state.write().await;
        let cluster = init_state.make_cluster_map();
        // dbg!(cluster.members());
        let mut cli = self.cluster_in_cli.clone();
        cli.set_new_cluster(cluster).await?;

        *writer = init_state;
        Ok(())
    }
    async fn process_read(&self, req: &[u8]) -> Result<Vec<u8>> {
        unimplemented!()
    }
    async fn fold_snapshot(
        &self,
        old_snapshot: Option<&[u8]>,
        entries: Vec<&[u8]>,
    ) -> Result<Vec<u8>> {
        unimplemented!()
    }
}
