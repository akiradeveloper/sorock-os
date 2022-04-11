use cmd_lib::*;
use lol_core::{api::*, RaftClient, Uri};
use sorock_core::proto_compiled::*;
use std::time::Duration;
use tonic::transport::{Channel, Endpoint};

const N: usize = 10;

#[derive(Clone)]
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

    for i in 0..3 {
        add_server(chan.clone(), node_list[i].clone()).await?;
    }

    for i in 0..N {
        let mut cli = sorock_client::SorockClient::new(chan.clone());
        cli.create(CreateReq {
            key: format!("key-{}", i),
            data: vec![0; 512].into(),
        })
        .await?;
    }
    run_sanity_check(chan.clone()).await?;

    // add nd3
    add_server(chan.clone(), node_list[3].clone()).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_cluster_size(chan.clone(), 4).await?;
    run_sanity_check(chan.clone()).await?;

    // stop nd1
    run_cmd!(docker-compose stop nd1)?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_cluster_size(chan.clone(), 3).await?;
    run_sanity_check(chan.clone()).await?;

    // restart nd1
    run_cmd!(docker-compose start nd1)?;
    add_server(chan.clone(), node_list[1].clone()).await?;
    tokio::time::sleep(Duration::from_secs(5)).await;
    assert_cluster_size(chan.clone(), 4).await?;
    run_sanity_check(chan.clone()).await?;

    run_cmd!(docker-compose down -v)?;
    Ok(())
}

async fn add_server(chan: Channel, node: Node) -> anyhow::Result<()> {
    let mut cli = RaftClient::new(chan.clone());
    let req = AddServerReq {
        id: node.id.to_string(),
    };
    cli.add_server(req).await?;

    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut cli = sorock_client::SorockClient::new(chan.clone());
    cli.add_node(AddNodeReq {
        uri: node.id.to_string(),
    })
    .await?;

    Ok(())
}

async fn run_sanity_check(chan: Channel) -> anyhow::Result<()> {
    for i in 0..N {
        let mut cli = sorock_client::SorockClient::new(chan.clone());
        let SanityCheckRep { n_lost } = cli
            .sanity_check(SanityCheckReq {
                key: format!("key-{}", i),
            })
            .await?
            .into_inner();

        anyhow::ensure!(n_lost == 0);
    }
    Ok(())
}

async fn assert_cluster_size(chan: Channel, should_be: usize) -> anyhow::Result<()> {
    let mut cli = RaftClient::new(chan.clone());
    let rep = cli
        .request_cluster_info(ClusterInfoReq {})
        .await?
        .into_inner();
    assert_eq!(rep.membership.len(), should_be);
    Ok(())
}
