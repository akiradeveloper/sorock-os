use crate::*;
use std::collections::HashSet;

#[norpc::service]
trait AppIn {
    // fn report_suspect();
    fn set_new_cluster(cluster: HashSet<Uri>) -> anyhow::Result<()>;
}
define_client!(AppIn);

pub fn spawn(queue_cli: queue::ClientT, reporter_cli: reporter::ClientT) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            queue_cli,
            reporter_cli,
        };
        let service = AppInService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    AppInClient::new(chan)
}

#[derive(Clone)]
struct App {
    queue_cli: queue::ClientT,
    reporter_cli: reporter::ClientT,
}
#[norpc::async_trait]
impl AppIn for App {
    async fn set_new_cluster(mut self, cluster: HashSet<Uri>) -> anyhow::Result<()> {
        self.queue_cli.set_new_cluster(cluster.clone()).await?;
        self.reporter_cli.set_new_cluster(cluster.clone()).await?;
        Ok(())
    }
}
