use crate::*;
use tonic::transport::Channel;

use lol_core::Uri;
use proto_compiled::sorock_client::SorockClient;
use proto_compiled::{
    IndexedPiece, PieceExistsReq, RequestAnyPiecesReq, RequestPieceReq, SendPieceReq,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[norpc::service]
trait PeerOut {
    fn send_piece(to: Uri, piece: SendPiece) -> std::result::Result<(), SendPieceError>;
    fn request_piece(to: Uri, loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>>;
    fn request_any_pieces(to: Uri, key: String) -> anyhow::Result<Vec<(u8, Vec<u8>)>>;
    fn piece_exists(to: Uri, loc: PieceLocator) -> anyhow::Result<bool>;
}
define_client!(PeerOut);

pub fn spawn(state: State) -> ClientT {
    use norpc::runtime::tokio::*;
    let svc = App { state };
    let svc = PeerOutService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
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

struct App {
    state: State,
}

#[norpc::async_trait]
impl PeerOut for App {
    async fn send_piece(
        &self,
        to: Uri,
        piece: SendPiece,
    ) -> std::result::Result<(), SendPieceError> {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        let rep = cli
            .send_piece(SendPieceReq {
                data: piece.data,
                key: piece.loc.key,
                index: piece.loc.index as u32,
                version: piece.version,
            })
            .await
            .map_err(|_| SendPieceError::Failed)?;
        let rep = rep.into_inner();
        match rep.error_code {
            0 => Ok(()),
            -1 => Err(SendPieceError::Rejected),
            -2 => Err(SendPieceError::Failed),
            _ => unreachable!(),
        }
    }
    async fn piece_exists(&self, to: Uri, loc: PieceLocator) -> anyhow::Result<bool> {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        let rep = cli
            .piece_exists(PieceExistsReq {
                key: loc.key,
                index: loc.index as u32,
            })
            .await?;
        let rep = rep.into_inner();
        Ok(rep.exists)
    }
    async fn request_piece(&self, to: Uri, loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>> {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        let rep = cli
            .request_piece(RequestPieceReq {
                key: loc.key,
                index: loc.index as u32,
            })
            .await?;
        let rep = rep.into_inner();
        Ok(rep.data)
    }
    async fn request_any_pieces(&self, to: Uri, key: String) -> anyhow::Result<Vec<(u8, Vec<u8>)>> {
        let chan = self.state.connect(to).await;
        let mut cli = SorockClient::new(chan);
        let rep = cli.request_any_pieces(RequestAnyPiecesReq { key }).await?;
        let rep = rep.into_inner();
        let mut out = vec![];
        for IndexedPiece { index, data } in rep.pieces {
            out.push((index as u8, data));
        }
        Ok(out)
    }
}
