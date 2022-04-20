use crate::*;
use futures::FutureExt;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

#[norpc::service]
trait Stabilizer {
    fn flush_queue();
    fn set_new_cluster(cluster: ClusterMap) -> anyhow::Result<()>;
    fn queue_task(task: StabilizeTask);
}
define_client!(Stabilizer);

pub fn spawn(
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
    state: State,
) -> ClientT {
    use norpc::runtime::tokio::*;
    let svc = App {
        piece_store_cli,
        peer_out_cli,
        state,
    };
    let svc = StabilizerService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    StabilizerClient::new(chan)
}

pub fn spawn_tick(mut stabilizer_cli: ClientT, interval: Duration) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(interval);
        loop {
            interval.tick().await;
            stabilizer_cli.flush_queue().await;
        }
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct StabilizeTask {
    pub key: String,
}

pub struct State {
    uri: Uri,
    cluster: RwLock<ClusterMap>,
    queue: RwLock<HashSet<StabilizeTask>>,
}
impl State {
    pub fn new(uri: Uri) -> Self {
        Self {
            uri,
            cluster: RwLock::new(ClusterMap::new()),
            queue: RwLock::new(HashSet::new()),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StabilizeError {
    #[error("send-piece with older version was rejected.")]
    Rejected,
    #[error("failed somehow")]
    Failed(String),
}

struct App {
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
    state: State,
}
#[norpc::async_trait]
impl Stabilizer for App {
    async fn queue_task(&self, task: StabilizeTask) {
        self.state.queue.write().await.insert(task);
    }
    async fn flush_queue(&self) {
        let this_uri = self.state.uri.clone();

        // Drain the current queue.
        let cur_queue: Vec<StabilizeTask> = self.state.queue.write().await.drain().collect();
        // eprintln!("uri: {}, flush_queue: len = {}", this_uri, cur_queue.len());

        let cur_cluster = self.state.cluster.read().await.clone();

        let futs = cur_queue.into_iter().map(|task| {
            let key = task.key;
            let exec = ExecStabilize {
                this_uri: this_uri.clone(),
                peer_out_cli: self.peer_out_cli.clone(),
                piece_store_cli: self.piece_store_cli.clone(),
                cur_cluster: cur_cluster.clone(),
            };
            exec.exec(key)
        });

        let mut failed_tasks = vec![];
        let stream = futures::stream::iter(futs);
        let n_par = std::thread::available_parallelism().unwrap().get() * 2;
        let mut buffered = stream.buffer_unordered(n_par);
        while let Some(rep) = buffered.next().await {
            match rep {
                Ok(()) => {}
                Err(StabilizeError::Rejected) => {
                    return;
                }
                Err(StabilizeError::Failed(key)) => {
                    let task = StabilizeTask { key };
                    failed_tasks.push(task);
                }
            }
        }
        drop(buffered);

        let mut queue = self.state.queue.write().await;
        for x in failed_tasks {
            queue.insert(x);
        }
    }
    async fn set_new_cluster(&self, new_cluster: ClusterMap) -> anyhow::Result<()> {
        *self.state.cluster.write().await = new_cluster;

        // Reset the queue
        let keys = self.piece_store_cli.clone().keys().await?;
        let mut init_queue = HashSet::new();
        for key in keys {
            init_queue.insert(StabilizeTask { key });
        }
        *self.state.queue.write().await = init_queue.into_iter().collect();

        Ok(())
    }
}

#[derive(Debug)]
struct MaybeMove {
    to: Uri,
    loc: PieceLocator,
}

struct ExecStabilize {
    this_uri: Uri,
    peer_out_cli: peer_out::ClientT,
    piece_store_cli: piece_store::ClientT,
    cur_cluster: ClusterMap,
}
impl ExecStabilize {
    async fn exec(self, key: String) -> std::result::Result<(), StabilizeError> {
        let placements = self.cur_cluster.compute_holders(key.clone(), N);
        // dbg!(&old_placement, &new_placement);
        let mut actions = vec![];
        for index in 0..N {
            let holder = &placements[index as usize];
            if let Some(holder) = holder {
                // Sending piece to myself will results in deleting the piece
                // because put and delete will happen in sequence.
                if holder != &self.this_uri {
                    let action = MaybeMove {
                        to: holder.clone(),
                        loc: PieceLocator {
                            key: key.clone(),
                            index: index as u8,
                        },
                    };
                    actions.push(action);
                }
            }
        }

        let mut futs = vec![];
        for MaybeMove { to, loc } in actions {
            let this_uri = self.this_uri.clone();
            let mut piece_store_cli = self.piece_store_cli.clone();
            let mut peer_out_cli = self.peer_out_cli.clone();
            let cluster_version = self.cur_cluster.version();
            let fut = async move {
                let data = piece_store_cli
                    .get_piece(loc.clone())
                    .await
                    .map_err(|_| SendPieceError::Failed)?;
                if let Some(data) = data {
                    // eprintln!("found send-piece some");
                    // Sending piece to myself will results in deleting the piece
                    // because put and delete will happen in sequence.
                    if to != this_uri {
                        peer_out_cli
                            .send_piece(
                                to,
                                SendPiece {
                                    version: cluster_version,
                                    loc: loc.clone(),
                                    data: Some(data.into()),
                                },
                            )
                            .await?;

                        piece_store_cli.delete_piece(loc).await.ok();

                        Ok(())
                    } else {
                        Ok(())
                    }
                } else {
                    // eprintln!("found send-piece none");
                    peer_out_cli
                        .send_piece(
                            to,
                            SendPiece {
                                version: cluster_version,
                                loc,
                                data: None,
                            },
                        )
                        .await?;

                    Ok(())
                }
            }
            .boxed();
            futs.push(fut);
        }

        let stream = futures::stream::iter(futs);
        let n_par = std::thread::available_parallelism().unwrap().get() * 2;
        let mut buffered = stream.buffer_unordered(n_par);
        while let Some(rep) = buffered.next().await {
            match rep {
                Ok(()) => {}
                Err(SendPieceError::Rejected) => return Err(StabilizeError::Rejected),
                _ => return Err(StabilizeError::Failed(key)),
            }
        }
        Ok(())
    }
}
