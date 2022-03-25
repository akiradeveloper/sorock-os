use crate::*;

use rebuild::Rebuild;
use stabilizer::StabilizeTask;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[norpc::service]
trait RebuildQueue {
    fn flush_queue();
    fn set_new_cluster(cluster: ClusterMap);
    fn queue_task(task: RebuildTask);
}
define_client!(RebuildQueue);

pub fn spawn(
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    state: State,
) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            piece_store_cli,
            peer_out_cli,
            stabilizer_cli,
            state: state.into(),
        };
        let service = RebuildQueueService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    RebuildQueueClient::new(chan)
}

pub fn spawn_tick(mut rebuild_queue_cli: ClientT, interval: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        loop {
            interval.tick().await;
            rebuild_queue_cli.flush_queue().await.ok();
        }
    });
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RebuildTask {
    pub loc: PieceLocator,
}
pub struct State {
    cluster: RwLock<ClusterMap>,
    queue: RwLock<HashSet<RebuildTask>>,
}
impl State {
    pub fn new() -> Self {
        Self {
            cluster: RwLock::new(ClusterMap::new()),
            queue: RwLock::new(HashSet::new()),
        }
    }
}

#[derive(Clone)]
struct App {
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    state: Arc<State>,
}
#[norpc::async_trait]
impl RebuildQueue for App {
    async fn flush_queue(self) {
        let cur_queue: Vec<RebuildTask> = self.state.queue.write().await.drain().collect();
        // eprintln!("flush_queue: len = {}", cur_queue.len());

        let cur_cluster = self.state.cluster.read().await.clone();

        let futs = cur_queue.into_iter().map(|task| {
            let loc = task.loc;
            let exec = ExecRebuild {
                peer_out_cli: self.peer_out_cli.clone(),
                piece_store_cli: self.piece_store_cli.clone(),
                stabilizer_cli: self.stabilizer_cli.clone(),
                cur_cluster: cur_cluster.clone(),
            };
            exec.exec(loc)
        });

        let mut failed_tasks = vec![];
        let stream = futures::stream::iter(futs);
        let n_par = std::thread::available_parallelism().unwrap().get() * 2;
        let mut buffered = stream.buffer_unordered(n_par);
        while let Some(rep) = buffered.next().await {
            match rep {
                Ok(()) => {}
                Err(RebuildError::Failed(loc)) => {
                    let task = RebuildTask { loc };
                    failed_tasks.push(task);
                }
            }
        }
        drop(buffered);

        // Requeue the failed tasks.
        let mut queue = self.state.queue.write().await;
        for x in failed_tasks {
            queue.insert(x);
        }
    }
    async fn set_new_cluster(self, cluster: ClusterMap) {
        *self.state.cluster.write().await = cluster;
    }
    async fn queue_task(self, task: RebuildTask) {
        self.state.queue.write().await.insert(task);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum RebuildError {
    #[error("failed")]
    Failed(PieceLocator),
}

struct ExecRebuild {
    cur_cluster: ClusterMap,
    stabilizer_cli: stabilizer::ClientT,
    peer_out_cli: peer_out::ClientT,
    piece_store_cli: piece_store::ClientT,
}
impl ExecRebuild {
    async fn exec(mut self, loc: PieceLocator) -> std::result::Result<(), RebuildError> {
        let check_exists = self
            .piece_store_cli
            .piece_exists(loc.clone())
            .await
            .unwrap()
            .map_err(|_| RebuildError::Failed(loc.clone()));
        match check_exists? {
            false => {
                let peer_out_cli = self.peer_out_cli.clone();

                // TODO: optimize
                // should first broadcast or ask the specific node for the wanting piece
                // before executing expensive rebuilding.

                let rebuild = Rebuild {
                    peer_out_cli,
                    cluster: self.cur_cluster,
                    with_parity: true,
                    fallback_broadcast: true,
                };
                let key = loc.key.clone();
                let mut pieces = rebuild
                    .rebuild(key)
                    .await
                    .map_err(|_| RebuildError::Failed(loc.clone()))?;
                let piece_data = pieces.swap_remove(loc.index as usize);
                self.piece_store_cli
                    .put_piece(loc.clone(), piece_data.into())
                    .await
                    .unwrap()
                    .map_err(|_| RebuildError::Failed(loc.clone()))?;

                self.stabilizer_cli
                    .queue_task(StabilizeTask { key: loc.key })
                    .await
                    .unwrap();

                Ok(())
            }
            true => Ok(()),
        }
    }
}
