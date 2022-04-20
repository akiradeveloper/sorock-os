use failure_detector::app_in as M;
use lol_core::Uri;
use std::collections::HashSet;

struct App;
#[norpc::async_trait]
impl M::AppIn for App {
    async fn set_new_cluster(&self, cluster: HashSet<Uri>) -> anyhow::Result<()> {
        Ok(())
    }
}
pub fn spawn() -> M::ClientT {
    use norpc::runtime::tokio::*;
    let svc = App {};
    let svc = M::AppInService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    M::AppInClient::new(chan)
}
