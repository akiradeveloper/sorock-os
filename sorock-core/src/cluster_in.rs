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
) -> ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            io_front_cli,
            stabilizer_cli,
            peer_in_cli,
            rebuild_queue_cli,
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
}

#[norpc::async_trait]
impl ClusterIn for App {
    async fn set_new_cluster(mut self, cluster: ClusterMap) -> anyhow::Result<()> {
        self.io_front_cli
            .set_new_cluster(cluster.clone())
            .await
            .unwrap();
        self.peer_in_cli
            .set_new_cluster(cluster.clone())
            .await
            .unwrap();
        self.rebuild_queue_cli
            .set_new_cluster(cluster.clone())
            .await
            .unwrap();
        self.stabilizer_cli
            .set_new_cluster(cluster)
            .await
            .unwrap()?;
        self.stabilizer_cli.flush_queue().await.unwrap();

        Ok(())
    }
}
