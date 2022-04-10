use cmd_lib::*;
use lol_core::{api::*, RaftClient, Uri};
use sorock_core::proto_compiled::*;
use std::time::Duration;
use tonic::transport::Endpoint;

const N: usize = 10;

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

    // Wait for all nodes to start up.
    tokio::time::sleep(Duration::from_secs(1)).await;

    let ep = Endpoint::new(node_list[0].dest.clone())?;
    let chan = ep.connect_lazy();

    for i in 0..2 {
        let mut cli = RaftClient::new(chan.clone());
        let req = AddServerReq {
            id: node_list[i].id.to_string(),
        };
        cli.add_server(req).await?;

        tokio::time::sleep(Duration::from_secs(1)).await;

        let mut cli = sorock_client::SorockClient::new(chan.clone());
        cli.add_node(AddNodeReq {
            uri: node_list[i].id.to_string(),
            cap: 1.,
        })
        .await?;
    }

    for i in 0..N {
        let mut cli = sorock_client::SorockClient::new(chan.clone());
        cli.create(CreateReq {
            key: format!("key-{}", i),
            data: vec![0; 512].into(),
        })
        .await?;
    }

    run_cmd!(docker-compose down -v)?;
    Ok(())
}
