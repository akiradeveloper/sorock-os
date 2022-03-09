#![deny(unused_must_use)]

use bytes::Bytes;
use lol_core::Uri;

#[macro_export]
macro_rules! define_client {
    ($name: ident) => {
        paste::paste! {
            pub type ClientT = [<$name Client>]<norpc::runtime::send::ClientService<[<$name Request>], [<$name Response>] >>;
        }
    };
}

mod safe_future;
use safe_future::into_safe_future;
mod def_services;
use def_services::*;
mod cluster_in;
mod cluster_map;
mod io_front;
mod peer_in;
mod peer_out;
mod piece_store;
mod stabilizer;
mod storage_service;
use cluster_map::ClusterMap;
mod rebuild;

mod raft_service;

#[cfg(test)]
mod tests;

use futures::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;

mod proto_compiled {
    tonic::include_proto!("sorock");
}

/// Paramter for erasure coding
/// K: # of data chunks
/// N-K: # of parity chunks
const K: usize = 4;
const N: usize = 8;

#[derive(serde::Serialize, serde::Deserialize)]
enum Command {
    AddNode { uri: URI, cap: f64 },
    RemoveNode { uri: URI },
}
impl Command {
    fn encode(&self) -> Vec<u8> {
        bincode::serialize(&self).unwrap()
    }
    fn decode(b: &[u8]) -> Self {
        bincode::deserialize(b).unwrap()
    }
}

#[derive(Clone, Hash, Debug)]
struct PieceLocator {
    key: String,
    index: u8,
}

struct SendPiece {
    version: u64,
    loc: PieceLocator,
    data: Option<Bytes>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Hash, PartialEq, Eq)]
pub struct URI(#[serde(with = "http_serde::uri")] tonic::transport::Uri);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut builder = tonic::transport::Server::builder();
    let socket = tokio::net::lookup_host("0.0.0.0:50000")
        .await
        .unwrap()
        .next()
        .expect("couldn't resolve socket address.");

    let server = todo!();
    let svc1 = storage_service::make_service(server).await;

    let mut signals = Signals::new(&[SIGTERM, SIGINT, SIGQUIT])?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        while let Some(signal) = signals.next().await {
            match signal {
                SIGTERM | SIGINT | SIGQUIT => {
                    tx.send(()).ok();
                    break;
                }
                _ => unreachable!(),
            }
        }
    });
    builder
        .add_service(svc1)
        .serve_with_shutdown(socket, async {
            rx.await.ok();
        })
        .await
        .expect("couldn't start the server.");

    Ok(())
}
