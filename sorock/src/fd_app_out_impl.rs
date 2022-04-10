use failure_detector as FD;
use std::sync::Arc;
use tonic::transport::{Channel, Uri};
use FD::app_out as M;

pub fn spawn(state: State) -> M::ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            state: state.into(),
        };
        let service = M::AppOutService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    M::AppOutClient::new(chan)
}

pub struct State {
    chan: Channel,
}
impl State {
    pub fn new(uri: Uri) -> Self {
        let e = tonic::transport::Endpoint::new(uri).unwrap();
        let chan = e.connect_lazy();
        Self { chan }
    }
}

#[derive(Clone)]
struct App {
    state: Arc<State>,
}

#[norpc::async_trait]
impl M::AppOut for App {
    async fn notify_failure(mut self, culprit: Uri) {
        eprintln!("{} is failed.", culprit);
        let mut cli1 =
            sorock_core::proto_compiled::sorock_client::SorockClient::new(self.state.chan.clone());
        cli1.remove_node(sorock_core::proto_compiled::RemoveNodeReq {
            uri: culprit.to_string(),
        })
        .await
        .unwrap();
        let mut cli2 = lol_core::RaftClient::new(self.state.chan.clone());
        cli2.remove_server(lol_core::api::RemoveServerReq {
            id: culprit.to_string(),
        })
        .await
        .unwrap();
    }
}
