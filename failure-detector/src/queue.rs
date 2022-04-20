use crate::*;
use futures::StreamExt;
use std::collections::HashSet;

#[norpc::service]
trait Queue {
    fn queue_suspect(suspect: Uri);
    fn set_new_cluster(cluster: HashSet<Uri>);
    fn run_once() -> anyhow::Result<()>;
}
define_client!(Queue);

pub fn spawn(
    peer_out_cli: peer_out::ClientT,
    app_out_cli: app_out::ClientT,
    state: State,
) -> ClientT {
    use norpc::runtime::tokio::*;
    let svc = App {
        peer_out_cli,
        app_out_cli,
        state: state.into(),
    };
    let svc = QueueService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    QueueClient::new(chan)
}

const K: usize = 3;

pub struct State {
    cluster: RwLock<HashSet<Uri>>,
    queue: RwLock<HashSet<Uri>>,
}
impl State {
    pub fn new() -> Self {
        Self {
            cluster: RwLock::new(HashSet::new()),
            queue: RwLock::new(HashSet::new()),
        }
    }
}

pub struct App {
    state: State,
    peer_out_cli: peer_out::ClientT,
    app_out_cli: app_out::ClientT,
}
#[norpc::async_trait]
impl Queue for App {
    async fn queue_suspect(&self, suspect: Uri) {
        self.state.queue.write().await.insert(suspect);
    }
    async fn set_new_cluster(&self, cluster: HashSet<Uri>) {
        *self.state.cluster.write().await = cluster;
    }
    async fn run_once(&self) -> anyhow::Result<()> {
        let mut writer = self.state.queue.write().await;
        let cur_list = writer.clone();
        *writer = HashSet::new();
        drop(writer);

        let mut all = self.state.cluster.read().await.clone();

        for suspect in cur_list {
            let mut peer_out_cli = self.peer_out_cli.clone();
            let mut app_out_cli = self.app_out_cli.clone();

            let removed = all.remove(&suspect);
            let ping2list = choose_unique_k(&all, K);
            if removed {
                all.insert(suspect.clone());
            }

            tokio::spawn(async move {
                let ok1 = peer_out_cli.ping1(suspect.clone()).await;
                if !ok1 {
                    let mut futs = vec![];
                    for proxy in ping2list {
                        let mut peer_out_cli = peer_out_cli.clone();
                        let suspect = suspect.clone();
                        let fut = async move { peer_out_cli.ping2(proxy, suspect).await };
                        futs.push(fut);
                    }
                    let stream = futures::stream::iter(futs);
                    let mut buffered = stream.buffer_unordered(K);
                    let mut ok2 = false;
                    while let Some(ping2rep) = buffered.next().await {
                        match ping2rep {
                            true => {
                                ok2 = true;
                                break;
                            }
                            _ => {}
                        }
                    }
                    if !ok2 {
                        let culprit = suspect;
                        app_out_cli.notify_failure(culprit).await.unwrap();
                    }
                }
            });
        }
        Ok(())
    }
}
fn choose_unique_k<T: Eq + std::hash::Hash + Clone>(set: &HashSet<T>, k: usize) -> HashSet<T> {
    use rand::seq::SliceRandom;
    let mut out = HashSet::new();
    let mut rng = rand::thread_rng();
    let m = std::cmp::min(set.len(), k);
    let mut v = vec![];
    for x in set {
        v.push(x.clone());
    }
    v.shuffle(&mut rng);
    for _ in 0..m {
        let x = v.pop().unwrap();
        out.insert(x);
    }
    out
}
#[test]
fn test_choose_unique_k() {
    let mut s = HashSet::new();
    for i in 1..=5 {
        s.insert(i);
    }
    dbg!(choose_unique_k(&s, 3));
    dbg!(choose_unique_k(&s, 3));
    dbg!(choose_unique_k(&s, 6));
}
