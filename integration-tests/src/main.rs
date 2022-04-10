use lol_core::{RaftClient, api::*, Uri};
use cmd_lib::*;
use tonic::transport::Endpoint;

struct Node {
    dest: Uri,
    id: Uri,
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut node_list = vec![];
    for i in 0..1000 {
        let node = Node {
            dest: Uri::from_maybe_shared(format!("http://localhost:5000{}", i))?,
            id: Uri::from_maybe_shared(format!("http://nd{}:50000", i))?,
        };
        node_list.push(node);
    }
    run_cmd!(docker-compose down -v)?;
    run_cmd!(docker-compose up -d)?;

    let ep = Endpoint::new(node_list[0].dest.clone())?;
    let chan = ep.connect_lazy();

    for i in 0..1 {
        let mut cli = RaftClient::new(chan.clone());
        let req = AddServerReq {
            id: node_list[i].id.to_string(),
        };
        cli.add_server(req).await?;
    }

    run_cmd!(docker-compose down -v)?;
    Ok(())
}
