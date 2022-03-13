use crate::*;

#[norpc::service]
trait Queue {
    fn queue_suspect();
    fn set_new_cluster();
    fn run_once();
}
define_client!(Queue);

#[derive(Clone)]
pub struct App {
    peer_out_cli: peer_out::ClientT,
    app_out_cli: app_out::ClientT,
}
#[norpc::async_trait]
impl Queue for App {
    async fn queue_suspect(self) {}
    async fn set_new_cluster(self) {}
    async fn run_once(self) {}
}
