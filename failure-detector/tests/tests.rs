use failure_detector::*;

use serial_test::serial;
use std::collections::{HashMap, HashSet};
use std::net::TcpListener;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tonic::transport::{Endpoint, Uri};

fn available_port() -> std::io::Result<u16> {
    TcpListener::bind("localhost:0").map(|x| x.local_addr().unwrap().port())
}

fn uri(port: u16) -> Uri {
    let uri = format!("http://localhost:{}", port);
    uri.parse().unwrap()
}

async fn start_server(port: u16, app_out_cli: app_out::ClientT, cluster: HashSet<Uri>) {
    let uri = uri(port);

    let peer_out_cli = peer_out::spawn(peer_out::State::new());
    let queue_cli = queue::spawn(peer_out_cli.clone(), app_out_cli, queue::State::new());
    let reporter_cli = reporter::spawn(queue_cli.clone(), reporter::State::new(uri.clone()));
    let mut app_in_cli = app_in::spawn(queue_cli.clone(), reporter_cli.clone());
    let svc = server::make_service(server::Server { peer_out_cli, uri }).await;

    app_in_cli.set_new_cluster(cluster).await.unwrap();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let mut reporter_cli = reporter_cli.clone();
        let mut queue_cli = queue_cli.clone();
        loop {
            interval.tick().await;
            reporter_cli.run_once().await.unwrap();
            queue_cli.run_once().await.unwrap();
        }
    });

    let mut builder = tonic::transport::Server::builder();
    let socket = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
    builder.add_service(svc).serve(socket).await.unwrap()
}

#[tokio::test]
#[serial]
async fn test_failure_detector() -> anyhow::Result<()> {
    let mut handles = HashMap::new();
    let mut ports = vec![];
    let mut cluster = HashSet::new();

    for _ in 0..10 {
        let port = available_port()?;
        ports.push(port);
        cluster.insert(uri(port));
    }

    let app = App::new();
    let app_out_cli = spawn(app.clone());
    for i in 0..10 {
        let port = ports[i];
        let hdl = tokio::spawn(start_server(port, app_out_cli.clone(), cluster.clone()));
        handles.insert(i, hdl);
    }
    // Wait for boot.
    tokio::time::sleep(Duration::from_secs(1)).await;

    for i in 0..10 {
        let port = ports[i];
        let uri = uri(port);
        let e = Endpoint::new(uri).unwrap();
        let connected = e.connect().await.is_ok();
        assert_eq!(connected, true);
    }

    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(app.q.read().unwrap().len(), 0);

    // Let's kill some processes.
    for i in [3, 5, 6] {
        let hdl = handles.get(&i).unwrap();
        hdl.abort();

        let port = ports[i];
        let uri = uri(port);
        let e = Endpoint::new(uri).unwrap();
        let connected = e.connect().await.is_ok();
        assert_eq!(connected, false);
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    // Should be detected.
    let cur_q = app.q.read().unwrap().clone();
    assert_eq!(cur_q.len(), 3);

    Ok(())
}

fn spawn(app: App) -> app_out::ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let service = app_out::AppOutService::new(app);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    app_out::AppOutClient::new(chan)
}
#[derive(Clone)]
struct App {
    q: Arc<RwLock<HashSet<Uri>>>,
}
impl App {
    fn new() -> Self {
        Self {
            q: Arc::new(RwLock::new(HashSet::new())),
        }
    }
}
#[norpc::async_trait]
impl app_out::AppOut for App {
    async fn notify_failure(self, calprit: Uri) {
        self.q.write().unwrap().insert(calprit);
    }
}
