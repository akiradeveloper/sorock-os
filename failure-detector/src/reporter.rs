use crate::*;
use std::collections::HashSet;

#[norpc::service]
trait Reporter {
    fn run_once() -> anyhow::Result<()>;
    fn set_new_cluster(cluster: HashSet<Uri>);
}
define_client!(Reporter);

pub fn spawn(queue_cli: queue::ClientT, state: State) -> ClientT {
    use norpc::runtime::tokio::*;
    let svc = App {
        queue_cli,
        state: state.into(),
    };
    let svc = ReporterService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    ReporterClient::new(chan)
}

pub struct State {
    uri: Uri,
    cluster: RwLock<HashSet<Uri>>,
}
impl State {
    pub fn new(uri: Uri) -> Self {
        Self {
            uri,
            cluster: RwLock::new(HashSet::new()),
        }
    }
}

struct App {
    queue_cli: queue::ClientT,
    state: State,
}
#[norpc::async_trait]
impl Reporter for App {
    async fn run_once(&self) -> anyhow::Result<()> {
        let mut candidates = vec![];
        let all = self.state.cluster.read().await.clone();
        for x in all {
            // Choose from except this node.
            if x != self.state.uri {
                candidates.push(x);
            }
        }
        let n = candidates.len();
        if n > 0 {
            let k = rand::random::<usize>() % n;
            let suspect = candidates.swap_remove(k);
            // dbg!(&suspect);
            self.queue_cli.clone().queue_suspect(suspect).await;
        }

        Ok(())
    }
    async fn set_new_cluster(&self, cluster: HashSet<Uri>) {
        *self.state.cluster.write().await = cluster;
    }
}
