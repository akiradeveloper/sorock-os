use crate::*;
use stabilizer::StabilizeTask;
use std::sync::Arc;
use tokio::sync::RwLock;

#[norpc::service]
trait PeerIn {
    fn set_new_cluster(cluster: ClusterMap);
    fn piece_exists(loc: PieceLocator) -> anyhow::Result<bool>;
    fn save_piece(piece: SendPiece) -> std::result::Result<(), SendPieceError>;
    fn find_piece(loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>>;
    fn find_any_pieces(key: String) -> anyhow::Result<Vec<(u8, Vec<u8>)>>;
}
define_client!(PeerIn);

pub fn spawn(
    piece_store_cli: piece_store::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    rebuild_queue_cli: rebuild_queue::ClientT,
    state: State,
) -> ClientT {
    use norpc::runtime::tokio::*;
    let svc = App {
        piece_store_cli,
        stabilizer_cli,
        rebuild_queue_cli,
        state,
    };
    let svc = PeerInService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
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
struct App {
    state: State,
    piece_store_cli: piece_store::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    rebuild_queue_cli: rebuild_queue::ClientT,
}
#[norpc::async_trait]
impl PeerIn for App {
    async fn set_new_cluster(&self, cluster: ClusterMap) {
        *self.state.cluster.write().await = cluster;
    }
    async fn piece_exists(&self, loc: PieceLocator) -> anyhow::Result<bool> {
        self.piece_store_cli.clone().piece_exists(loc).await
    }
    async fn save_piece(&self, send_piece: SendPiece) -> std::result::Result<(), SendPieceError> {
        let cluster = self.state.cluster.read().await;
        let this_version = cluster.version();
        if this_version > send_piece.version {
            return Err(SendPieceError::Rejected);
        }
        let loc = send_piece.loc;
        match send_piece.data {
            Some(data) => {
                self.piece_store_cli
                    .clone()
                    .put_piece(loc.clone(), data)
                    .await
                    .map_err(|_| SendPieceError::Failed)?;
                self.stabilizer_cli
                    .clone()
                    .queue_task(StabilizeTask { key: loc.key })
                    .await;

                Ok(())
            }
            None => {
                let task = rebuild_queue::RebuildTask { loc };
                self.rebuild_queue_cli.clone().queue_task(task).await;
                Ok(())
            }
        }
    }
    async fn find_piece(&self, loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>> {
        let piece = self.piece_store_cli.clone().get_piece(loc).await?;
        Ok(piece)
    }
    async fn find_any_pieces(&self, key: String) -> anyhow::Result<Vec<(u8, Vec<u8>)>> {
        let pieces = self
            .piece_store_cli
            .clone()
            .get_pieces(key, N as u8)
            .await?;
        Ok(pieces)
    }
}
