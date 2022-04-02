use sorock_core::piece_store::mem as mem_piece_store;
use sorock_core::*;

use bytes::Bytes;
use lol_core::Uri;
use serial_test::serial;
use std::collections::HashMap;
use std::net::TcpListener;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;
use tokio::task::JoinHandle;
use tonic::transport::Channel;

fn available_port() -> std::io::Result<u16> {
    TcpListener::bind("localhost:0").map(|x| x.local_addr().unwrap().port())
}

fn uri(port: u16) -> Uri {
    let uri = format!("http://localhost:{}", port);
    uri.parse().unwrap()
}

async fn start_server(port: u16) {
    let socket = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    let uri = uri(port);
    let peer_out_cli = peer_out::spawn(peer_out::State::new());
    let io_front_cli = io_front::spawn(peer_out_cli.clone(), io_front::State::new());
    // let piece_store_cli = mem_piece_store::spawn(mem_piece_store::State::new());
    let piece_store_cli = piece_store::sqlite::spawn(piece_store::sqlite::State::new(piece_store::sqlite::StoreType::Memory).await);
    let stabilizer_cli = stabilizer::spawn(
        piece_store_cli.clone(),
        peer_out_cli.clone(),
        stabilizer::State::new(uri.clone()),
    );
    stabilizer::spawn_tick(stabilizer_cli.clone(), Duration::from_millis(100));
    let rebuild_queue_cli = rebuild_queue::spawn(
        piece_store_cli.clone(),
        peer_out_cli.clone(),
        stabilizer_cli.clone(),
        rebuild_queue::State::new(),
    );
    rebuild_queue::spawn_tick(rebuild_queue_cli.clone(), Duration::from_millis(500));
    let peer_in_cli = peer_in::spawn(
        piece_store_cli,
        stabilizer_cli.clone(),
        rebuild_queue_cli.clone(),
        peer_in::State::new(),
    );
    let server =
        storage_service::Server::new(io_front_cli.clone(), peer_in_cli.clone(), uri.clone());
    let svc1 = storage_service::make_service(server).await;

    let cluster_in_cli =
        cluster_in::spawn(io_front_cli, stabilizer_cli, peer_in_cli, rebuild_queue_cli);
    let raft_app = raft_service::App::new(cluster_in_cli);
    let raft_app =
        lol_core::simple::ToRaftApp::new(raft_app, lol_core::simple::BytesRepository::new());
    let config = lol_core::ConfigBuilder::default()
        .compaction_interval_sec(0)
        .build()
        .unwrap();
    let svc2 = lol_core::make_raft_service(
        raft_app,
        lol_core::storage::memory::Storage::new(),
        uri,
        config,
    )
    .await;

    let mut builder = tonic::transport::Server::builder();
    builder
        .add_service(svc1)
        .add_service(svc2)
        .serve(socket)
        .await
        .expect("failed to start a server");
}

struct Cluster {
    servers: HashMap<Uri, JoinHandle<()>>,
    leader: Option<Uri>,
    conn: Option<Channel>,
}
impl Cluster {
    fn new() -> Self {
        Self {
            servers: HashMap::new(),
            leader: None,
            conn: None,
        }
    }
    async fn connect(&self) -> tonic::transport::Channel {
        self.conn.as_ref().unwrap().clone()
    }
    /// Choose non-leader node from the existing nodes.
    fn choose_one(&self) -> Uri {
        let leader = self.leader.as_ref().unwrap();
        let mut uris = vec![];
        for (k, _) in &self.servers {
            if k != leader {
                uris.push(k.clone());
            }
        }
        let i = rand::random::<usize>() % uris.len();
        uris[i].clone()
    }
    async fn up_node(&mut self) -> Uri {
        let port = loop {
            // Getting a new port number often fails in high load but
            // should be successful in several more retries.
            if let Ok(x) = available_port() {
                break x;
            }
        };
        let add_uri = uri(port);
        eprintln!("add node {}", &add_uri);
        let hdl = tokio::spawn(start_server(port));
        tokio::time::sleep(Duration::from_millis(100)).await;

        let n = self.servers.len();
        if n == 0 {
            self.leader = Some(add_uri.clone());
            let e = tonic::transport::Endpoint::new(add_uri.clone()).unwrap();
            let chan = e.connect().await.unwrap();
            self.conn = Some(chan);
        }
        let chan = self.connect().await;
        let mut cli = lol_core::RaftClient::new(chan);
        loop {
            let rep = cli
                .add_server(lol_core::api::AddServerReq {
                    id: add_uri.to_string(),
                })
                .await;
            if rep.is_ok() {
                break;
            } else {
                tokio::task::yield_now().await;
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.servers.insert(add_uri.clone(), hdl);

        add_uri
    }
    async fn down_node(&mut self, uri: Uri) {
        let chan = self.connect().await;
        let mut cli = lol_core::RaftClient::new(chan);
        loop {
            let rep = cli
                .remove_server(lol_core::api::RemoveServerReq {
                    id: uri.to_string(),
                })
                .await;
            if rep.is_ok() {
                break;
            } else {
                tokio::task::yield_now().await;
            }
        }
        let hdl = self.servers.remove(&uri).unwrap();
        hdl.abort();
    }
    async fn add_node(&self, uri: Uri, cap: f64) {
        let chan = self.connect().await;
        let mut cli = proto_compiled::sorock_client::SorockClient::new(chan);
        let req = proto_compiled::AddNodeReq {
            uri: uri.to_string(),
            cap,
        };
        cli.add_node(req).await.unwrap();
    }
    async fn remove_node(&self, uri: Uri) {
        eprintln!("remove node {}", &uri);
        let chan = self.connect().await;
        let mut cli = proto_compiled::sorock_client::SorockClient::new(chan);
        let req = proto_compiled::RemoveNodeReq {
            uri: uri.to_string(),
        };
        cli.remove_node(req).await.unwrap();
    }
    async fn create(&self, key: &str, value: &[u8]) {
        let chan = self.connect().await;
        let mut cli = proto_compiled::sorock_client::SorockClient::new(chan);
        let req = proto_compiled::CreateReq {
            key: key.to_string(),
            data: Bytes::copy_from_slice(value),
        };
        cli.create(req).await.unwrap();
    }
    async fn read(&self, key: &str) -> Vec<u8> {
        let chan = self.connect().await;
        let mut cli = proto_compiled::sorock_client::SorockClient::new(chan);
        let req = proto_compiled::ReadReq {
            key: key.to_string(),
        };
        let rep = cli.read(req).await.unwrap().into_inner();
        let mut out = vec![];
        out.extend_from_slice(&rep.data);
        out
    }
    async fn sanity_check(&self, key: &str) -> u8 {
        let chan = self.connect().await;
        let mut cli = proto_compiled::sorock_client::SorockClient::new(chan);
        let req = proto_compiled::SanityCheckReq {
            key: key.to_string(),
        };
        let rep = cli.sanity_check(req).await.unwrap().into_inner();
        rep.n_lost as u8
    }
    async fn delete(&self, key: &str) {
        unimplemented!()
    }
}

#[tokio::test]
#[serial]
async fn test_add_1_node() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    let uri = cluster.up_node().await;
    cluster.add_node(uri.clone(), 1.0).await;
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ping() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    let uri = cluster.up_node().await;
    cluster.add_node(uri.clone(), 1.0).await;

    let chan = cluster.connect().await;
    let mut cli = proto_compiled::sorock_client::SorockClient::new(chan);

    assert!(cli.ping(()).await.is_ok());

    cluster.remove_node(uri.clone()).await;
    cluster.down_node(uri.clone()).await;
    assert!(tonic::transport::Endpoint::new(uri)?
        .connect()
        .await
        .is_err());

    Ok(())
}

fn prepare_dataset(n: usize) -> Vec<(String, Vec<u8>)> {
    let mut out = vec![];
    for _ in 0..n {
        let mut v = vec![0; 16];
        for i in 0..16 {
            let x = rand::random::<u8>();
            v[i] = x;
        }
        let k = md5::compute(&v);
        let k = format!("{:x}", k);
        out.push((k, v));
    }
    out
}

#[tokio::test]
#[serial]
async fn test_io_1_node() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    let uri = cluster.up_node().await;
    cluster.add_node(uri, 1.0).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    for (k, v) in &dataset {
        let read = cluster.read(k).await;
        assert_eq!(&read, v);
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_add_3_node() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..3 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_add_10_node() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_add_remove_10_node() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    for _ in 0..20 {
        let remove_uri = cluster.choose_one();
        cluster.remove_node(remove_uri.clone()).await;
        cluster.down_node(remove_uri).await;

        let add_uri = cluster.up_node().await;
        cluster.add_node(add_uri, 1.0).await;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_io_10_node() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(1000);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    for (k, v) in &dataset {
        let read = cluster.read(k).await;
        assert_eq!(&read, v);
    }
    for (k, _) in &dataset {
        let n_lost = cluster.sanity_check(k).await;
        assert_eq!(n_lost, 0);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn text_expand_once() {
    let mut cluster = Cluster::new();
    let uri = cluster.up_node().await;
    cluster.add_node(uri, 1.0).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    let uri = cluster.up_node().await;
    cluster.add_node(uri, 1.0).await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    eprintln!("stabilized.");

    for (k, _) in &dataset {
        let n_lost = cluster.sanity_check(k).await;
        assert_eq!(n_lost, 0);
    }
    for (k, v) in &dataset {
        let read = cluster.read(k).await;
        assert_eq!(&read, v);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn text_expand_once_10_node() {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    let uri = cluster.up_node().await;
    cluster.add_node(uri, 1.0).await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    eprintln!("stabilized.");

    for (k, _) in &dataset {
        let n_lost = cluster.sanity_check(k).await;
        assert_eq!(n_lost, 0);
    }
    for (k, v) in &dataset {
        let read = cluster.read(k).await;
        assert_eq!(&read, v);
    }
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_shrink_once() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    let uri = cluster.choose_one();
    cluster.remove_node(uri.clone()).await;
    cluster.down_node(uri).await;
    tokio::time::sleep(Duration::from_secs(10)).await;
    eprintln!("stabilized.");

    for (k, _) in &dataset {
        let n_lost = cluster.sanity_check(k).await;
        assert_eq!(n_lost, 0);
    }
    for (k, v) in &dataset {
        let read = cluster.read(k).await;
        assert_eq!(&read, v);
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_expanding_cluster() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..1 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    for _ in 1..10 {
        for (k, v) in &dataset {
            let read = cluster.read(k).await;
            assert_eq!(&read, v);
        }
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_shrinking_cluster() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    for _ in 0..7 {
        for (k, v) in &dataset {
            let read = cluster.read(k).await;
            assert_eq!(&read, v);
        }
        let uri = cluster.choose_one();
        cluster.remove_node(uri.clone()).await;
        cluster.down_node(uri).await;

        // FIXME should strictly wait for stabilization.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
#[serial]
async fn test_changing_cluster() -> anyhow::Result<()> {
    let mut cluster = Cluster::new();
    for _ in 0..10 {
        let uri = cluster.up_node().await;
        cluster.add_node(uri, 1.0).await;
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    let dataset = prepare_dataset(100);
    for (k, v) in &dataset {
        cluster.create(k, v).await;
    }

    for _ in 0..5 {
        for (k, v) in &dataset {
            let read = cluster.read(k).await;
            assert_eq!(&read, v);
        }

        // remove a node.
        let remove_uri = cluster.choose_one();
        cluster.remove_node(remove_uri.clone()).await;
        cluster.down_node(remove_uri).await;

        // FIXME should strictly wait for stabilization.
        tokio::time::sleep(Duration::from_secs(1)).await;

        // add a new node.
        let add_uri = cluster.up_node().await;
        cluster.add_node(add_uri, 1.0).await;

        // FIXME should strictly wait for stabilization.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    Ok(())
}
