use failure_detector::app_in as M;
use lol_core::Uri;
use std::collections::HashSet;

#[derive(Clone)]
struct App;
#[norpc::async_trait]
impl M::AppIn for App {
	async fn set_new_cluster(mut self, cluster: HashSet<Uri>) {}
}
pub fn spawn() -> M::ClientT {
	use norpc::runtime::send::*;
	let (tx, rx) = tokio::sync::mpsc::channel(100);
	tokio::spawn(async {
		let svc = App {};
		let service = M::AppInService::new(svc);
		let server = ServerExecutor::new(rx, service);
		server.serve().await
	});
	let chan = ClientService::new(tx);
	M::AppInClient::new(chan)
}