use crate::*;
use futures::FutureExt;
use std::sync::Arc;
use tokio::sync::RwLock;

#[norpc::service]
trait Stabilizer {
    fn set_new_cluster(cluster: ClusterMap);
}
define_client!(Stabilizer);

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
        let service = StabilizerService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    StabilizerClient::new(chan)
}

pub struct State {
    uri: Uri,
    cluster: RwLock<ClusterMap>,
}
impl State {
    pub fn new(uri: Uri) -> Self {
        Self {
            uri,
            cluster: RwLock::new(ClusterMap::new()),
        }
    }
}

#[derive(Clone)]
struct App {
    piece_store_cli: piece_store::ClientT,
    peer_out_cli: peer_out::ClientT,
    state: Arc<State>,
}
#[norpc::async_trait]
impl Stabilizer for App {
    async fn set_new_cluster(mut self, new_cluster: ClusterMap) {
        let mut cur_cluster = self.state.cluster.write().await;
        let old_cluster = cur_cluster.clone();

        let this_uri = &self.state.uri;
        let last_change = compute_last_change(&old_cluster, &new_cluster);

        let peer_out_cli = self.peer_out_cli.clone();
        let mut piece_store_cli = self.piece_store_cli.clone();
        let keys = piece_store_cli.keys().await.unwrap();
        let futs = keys.into_iter().map(|key| {
            let stabilize = Stabilize {
                peer_out_cli: peer_out_cli.clone(),
                piece_store_cli: piece_store_cli.clone(),
                old_cluster: old_cluster.clone(),
                new_cluster: new_cluster.clone(),
                last_change: last_change.clone(),
                uri: this_uri.clone(),
            };
            stabilize.stabilize(key)
        });
        let stream = futures::stream::iter(futs);
        let mut buffered = stream.buffer_unordered(100);
        while let Some(_) = buffered.next().await {}
        drop(buffered);

        *cur_cluster = new_cluster;
    }
}

#[derive(Debug)]
enum Action {
    SelfHeal { from: Uri, loc: PieceLocator },
    MoveOwnership { to: Uri, loc: PieceLocator },
    PseudoMove { to: Uri, loc: PieceLocator },
}

#[derive(Clone, Debug)]
enum Change {
    Add(Uri),
    Remove(Uri),
}
fn compute_last_change(old: &ClusterMap, new: &ClusterMap) -> Change {
    let mut xx = old.members();
    let mut yy = new.members();
    if yy.len() > xx.len() {
        assert_eq!(yy.len(), xx.len() + 1);
        for x in xx {
            yy.remove(&x);
        }
        let y = yy.into_iter().last().unwrap();
        Change::Add(y)
    } else if xx.len() > yy.len() {
        assert_eq!(xx.len(), yy.len() + 1);
        for y in yy {
            xx.remove(&y);
        }
        let x = xx.into_iter().last().unwrap();
        Change::Remove(x)
    } else {
        unreachable!()
    }
}

struct Stabilize {
    peer_out_cli: peer_out::ClientT,
    piece_store_cli: piece_store::ClientT,
    old_cluster: ClusterMap,
    new_cluster: ClusterMap,
    last_change: Change,
    uri: Uri,
}
impl Stabilize {
    async fn stabilize(self, key: String) {
        let old_placement = self.old_cluster.compute_holders(key.clone(), N);
        let new_placement = self.new_cluster.compute_holders(key.clone(), N);
        // dbg!(&old_placement, &new_placement);
        let mut actions = vec![];
        for index in 0..N {
            let old = &old_placement[index as usize];
            let new = &new_placement[index as usize];
            match (old, new, &self.last_change) {
                // src node is removed.
                // This incurs "pseudo move" that mimicks as if the removed node moves
                // a piece but without the data that will be rebuilt in the receiver-side.
                (Some(old), Some(new), Change::Remove(ref removed)) if removed == old => {
                    let action = Action::PseudoMove {
                        to: new.clone(),
                        loc: PieceLocator {
                            key: key.clone(),
                            index: index as u8,
                        },
                    };
                    actions.push(action);
                }
                // Any change directs to this node will incurs self healing
                // because the change means the piece should be owned by this node.
                (Some(old), Some(new), _) if new == &self.uri => {
                    let action = Action::SelfHeal {
                        from: old.clone(),
                        loc: PieceLocator {
                            key: key.clone(),
                            index: index as u8,
                        },
                    };
                    actions.push(action);
                }
                // Any change directs to other node will incurs move.
                // If the piece is found in this node, the piece should be moved the computed holder.
                (_, Some(new), _) if new != &self.uri => {
                    let action = Action::MoveOwnership {
                        to: new.clone(),
                        loc: PieceLocator {
                            key: key.clone(),
                            index: index as u8,
                        },
                    };
                    actions.push(action);
                }
                _ => {}
            }
        }

        let mut futs = vec![];
        for action in actions {
            let fut = match action {
                Action::SelfHeal { from, loc } => {
                    let mut piece_store_cli = self.piece_store_cli.clone();
                    let mut peer_out_cli = self.peer_out_cli.clone();
                    let new_cluster = self.new_cluster.clone();
                    into_safe_future(async move {
                        let data = piece_store_cli.get_piece(loc.clone()).await.unwrap();
                        if data.is_none() {
                            let piece =
                                peer_out_cli.request_piece(from, loc.clone()).await.unwrap();
                            if let Some(piece) = piece {
                                piece_store_cli.put_piece(loc, piece.into()).await.unwrap();
                            } else {
                                let rebuild = rebuild::Rebuild {
                                    cluster: new_cluster.clone(),
                                    peer_out_cli: peer_out_cli.clone(),
                                    with_parity: true,
                                    fallback_broadcast: true,
                                };
                                if let Some(mut pieces) = rebuild.rebuild(loc.key.clone()).await {
                                    let piece = pieces.swap_remove(loc.index as usize);
                                    piece_store_cli.put_piece(loc, piece.into()).await.unwrap();
                                }
                            }
                        }
                    })
                    .boxed()
                }
                Action::MoveOwnership { to, loc } => {
                    let mut piece_store_cli = self.piece_store_cli.clone();
                    let mut peer_out_cli = self.peer_out_cli.clone();
                    let cluster_version = self.new_cluster.version();
                    into_safe_future(async move {
                        let data = piece_store_cli.get_piece(loc.clone()).await.unwrap();
                        if let Some(data) = data {
                            peer_out_cli
                                .send_piece(
                                    to,
                                    SendPiece {
                                        version: cluster_version,
                                        loc: loc.clone(),
                                        data: Some(data.into()),
                                    },
                                )
                                .await
                                .unwrap();

                            piece_store_cli.delete_piece(loc).await.unwrap();
                        }
                    })
                    .boxed()
                }
                Action::PseudoMove { to, loc } => {
                    let mut peer_out_cli = self.peer_out_cli.clone();
                    let cluster_version = self.new_cluster.version();
                    into_safe_future(async move {
                        peer_out_cli
                            .send_piece(
                                to,
                                SendPiece {
                                    version: cluster_version,
                                    loc,
                                    data: None,
                                },
                            )
                            .await
                            .ok();
                    })
                    .boxed()
                }
            };
            futs.push(fut);
        }

        let stream = futures::stream::iter(futs);
        let n_par = std::thread::available_parallelism().unwrap().get() * 2;
        let mut buffered = stream.buffer_unordered(n_par);
        while let Some(_) = buffered.next().await {}
    }
}
