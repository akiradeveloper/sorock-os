use crate::*;

use std::collections::HashMap;
use tonic::transport::Channel;

#[norpc::service]
trait PeerOut {
    fn ping1(tgt: Uri) -> bool;
    fn ping2(to: Uri, tgt: Uri) -> bool;
}
define_client!(PeerOut);

pub fn spawn(state: State) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            state: state.into(),
        };
        let service = PeerOutService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    PeerOutClient::new(chan)
}

pub struct State {
    conn_cache: RwLock<HashMap<Uri, Channel>>,
}
impl State {
    pub fn new() -> Self {
        Self {
            conn_cache: RwLock::new(HashMap::new()),
        }
    }
    async fn connect(&self, uri: Uri) -> Channel {
        if !self.conn_cache.read().await.contains_key(&uri) {
            let new_chan = {
                let e = tonic::transport::Endpoint::new(uri.clone()).unwrap();
                let chan = e.connect_lazy();
                chan
            };
            self.conn_cache.write().await.insert(uri.clone(), new_chan);
        }
        self.conn_cache.read().await.get(&uri).unwrap().clone()
    }
}

#[derive(Clone)]
struct App {
    state: Arc<State>,
}
#[norpc::async_trait]
impl PeerOut for App {
    async fn ping1(self, tgt: Uri) -> bool {
        let e = tonic::transport::Endpoint::new(tgt.clone()).unwrap();
        let connected = e.connect().await.is_ok();
        if !connected {
            return false;
        }
        let chan = self.state.connect(tgt).await;
        let mut cli = proto_compiled::fd_client::FdClient::new(chan);
        let req = proto_compiled::Ping1Req {};
        cli.ping1(req).await.is_ok()
    }
    async fn ping2(self, to: Uri, tgt: Uri) -> bool {
        let e = tonic::transport::Endpoint::new(to.clone()).unwrap();
        let connected = e.connect().await.is_ok();
        if !connected {
            return false;
        }

        let chan = self.state.connect(to).await;
        let mut cli = proto_compiled::fd_client::FdClient::new(chan);
        let req = proto_compiled::Ping2Req {
            suspect_uri: tgt.to_string(),
        };
        let rep = cli.ping2(req).await.unwrap();
        let proto_compiled::Ping2Rep { ok } = rep.into_inner();
        ok
    }
}
