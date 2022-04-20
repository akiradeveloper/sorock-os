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
    use norpc::runtime::tokio::*;
    let svc = App {
        io_front_cli,
        stabilizer_cli,
        peer_in_cli,
        rebuild_queue_cli,
        fd_app_in_cli,
    };
    let svc = ClusterInService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    ClusterInClient::new(chan)
}

struct App {
    io_front_cli: io_front::ClientT,
    stabilizer_cli: stabilizer::ClientT,
    rebuild_queue_cli: rebuild_queue::ClientT,
    peer_in_cli: peer_in::ClientT,
    fd_app_in_cli: failure_detector::app_in::ClientT,
}

#[norpc::async_trait]
impl ClusterIn for App {
    async fn set_new_cluster(&self, cluster: ClusterMap) -> anyhow::Result<()> {
        self.fd_app_in_cli
            .clone()
            .set_new_cluster(cluster.members())
            .await?;
        self.io_front_cli
            .clone()
            .set_new_cluster(cluster.clone())
            .await;
        self.peer_in_cli
            .clone()
            .set_new_cluster(cluster.clone())
            .await;
        self.rebuild_queue_cli
            .clone()
            .set_new_cluster(cluster.clone())
            .await;
        self.stabilizer_cli.clone().set_new_cluster(cluster).await?;
        self.stabilizer_cli.clone().flush_queue().await;

        Ok(())
    }
}
