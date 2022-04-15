use crate::*;

#[norpc::service]
trait ClusterIn {
    fn set_new_cluster(cluster: ClusterMap) -> anyhow::Result<()>;
}
define_client!(ClusterIn);

pub fn spawn(
    io_front_cli: io_front::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    peer_in_cli: peer_in::ClientT,
    rebuild_queue_cli: rebuild_queue::ClientT,
    fd_app_in_cli: failure_detector::app_in::ClientT,
) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            io_front_cli,
            stabilizer_cli,
            peer_in_cli,
            rebuild_queue_cli,
            fd_app_in_cli,
        };
        let service = ClusterInService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    ClusterInClient::new(chan)
}

#[derive(Clone)]
struct App {
    io_front_cli: io_front::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    rebuild_queue_cli: rebuild_queue::ClientT,
    peer_in_cli: peer_in::ClientT,
    fd_app_in_cli: failure_detector::app_in::ClientT,
}

#[norpc::async_trait]
impl ClusterIn for App {
    async fn set_new_cluster(mut self, cluster: ClusterMap) -> anyhow::Result<()> {
        self.fd_app_in_cli
            .set_new_cluster(cluster.members())
            .await?;
        self.io_front_cli
            .set_new_cluster(cluster.clone())
            .await?;
        self.peer_in_cli
            .set_new_cluster(cluster.clone())
            .await?;
        self.rebuild_queue_cli
            .set_new_cluster(cluster.clone())
            .await?;
        self.stabilizer_cli
            .set_new_cluster(cluster)
            .await??;
        self.stabilizer_cli.flush_queue().await?;

        Ok(())
    }
}
