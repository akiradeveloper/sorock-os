use crate::*;
use rebuild::Rebuild;
use std::sync::Arc;
use tokio::sync::RwLock;

define_client!(PeerIn);
pub fn spawn(
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
    state: State,
) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            piece_store_cli,
            peer_out_cli,
            state: Arc::new(state),
        };
        let service = PeerInService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    PeerInClient::new(chan)
}

pub struct State {
    cluster: RwLock<ClusterMap>,
}
impl State {
    pub fn new() -> Self {
        Self {
            cluster: RwLock::new(ClusterMap::new()),
        }
    }
}
#[derive(Clone)]
struct App {
    state: Arc<State>,
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
}
#[norpc::async_trait]
impl PeerIn for App {
    async fn set_new_cluster(self, cluster: ClusterMap) {
        *self.state.cluster.write().await = cluster;
    }
    async fn save_piece(mut self, send_piece: SendPiece) {
        let cluster = self.state.cluster.read().await;
        let this_version = cluster.version();
        if this_version > send_piece.version {
            panic!(
                "piece in old version is denied. key={} {} < {}",
                &send_piece.loc.key, send_piece.version, this_version,
            );
        }
        match send_piece.data {
            Some(data) => {
                self.piece_store_cli
                    .put_piece(send_piece.loc, data)
                    .await
                    .unwrap();
            }
            None => {
                let peer_out_cli = self.peer_out_cli.clone();
                let rebuild = Rebuild {
                    peer_out_cli,
                    cluster: cluster.clone(),
                    with_parity: true,
                    fallback_broadcast: true,
                };
                let key = send_piece.loc.key.clone();
                let pieces = rebuild.rebuild(key).await;
                if let Some(mut pieces) = pieces {
                    let piece_data = pieces.swap_remove(send_piece.loc.index as usize);
                    self.piece_store_cli
                        .put_piece(send_piece.loc, piece_data.into())
                        .await
                        .unwrap();
                }
            }
        }
    }
    async fn find_piece(mut self, loc: PieceLocator) -> Option<Vec<u8>> {
        self.piece_store_cli.get_piece(loc).await.unwrap()
    }
    async fn find_any_pieces(mut self, key: String) -> Vec<(u8, Vec<u8>)> {
        let mut out = vec![];
        for index in 0..N {
            let loc = PieceLocator {
                key: key.clone(),
                index: index as u8,
            };
            let found_piece = self.piece_store_cli.get_piece(loc).await.unwrap_or(None);
            if let Some(piece) = found_piece {
                out.push((index as u8, piece));
            }
        }
        out
    }
}
