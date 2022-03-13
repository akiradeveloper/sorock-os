use crate::*;
use tonic::transport::Channel;

use proto_compiled::sorock_client::SorockClient;
use proto_compiled::{IndexedPiece, RequestAnyPiecesReq, RequestPieceReq, SendPieceReq};
use lol_core::Uri;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

define_client!(PeerOut);
pub fn spawn(state: State) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            state: Arc::new(state),
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
    async fn send_piece(self, to: Uri, piece: SendPiece) {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        cli.send_piece(SendPieceReq {
            data: piece.data,
            key: piece.loc.key,
            index: piece.loc.index as u32,
            version: piece.version,
        })
        .await
        .unwrap();
    }
    async fn request_piece(self, to: Uri, loc: PieceLocator) -> Option<Vec<u8>> {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        let rep = cli
            .request_piece(RequestPieceReq {
                key: loc.key,
                index: loc.index as u32,
            })
            .await
            .unwrap();
        let rep = rep.into_inner();
        rep.data
    }
    async fn request_any_pieces(self, to: Uri, key: String) -> Vec<(u8, Vec<u8>)> {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        let rep = cli
            .request_any_pieces(RequestAnyPiecesReq { key })
            .await
            .unwrap();
        let rep = rep.into_inner();
        let mut out = vec![];
        for IndexedPiece { index, data } in rep.pieces {
            out.push((index as u8, data));
        }
        out
    }
}
