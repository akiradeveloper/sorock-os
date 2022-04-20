use crate::*;
use std::collections::HashSet;

#[norpc::service]
trait AppIn {
    // fn report_suspect();
    fn set_new_cluster(cluster: HashSet<Uri>) -> anyhow::Result<()>;
}
define_client!(AppIn);

pub fn spawn(queue_cli: queue::ClientT, reporter_cli: reporter::ClientT) -> ClientT {
    use norpc::runtime::tokio::*;
    let svc = App {
        queue_cli,
        reporter_cli,
    };
    let svc = AppInService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    AppInClient::new(chan)
}

struct App {
    queue_cli: queue::ClientT,
    reporter_cli: reporter::ClientT,
}
#[norpc::async_trait]
impl AppIn for App {
    async fn set_new_cluster(&self, cluster: HashSet<Uri>) -> anyhow::Result<()> {
        self.queue_cli
            .clone()
            .set_new_cluster(cluster.clone())
            .await;
        self.reporter_cli
            .clone()
            .set_new_cluster(cluster.clone())
            .await;
        Ok(())
    }
}
