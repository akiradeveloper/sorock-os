#![deny(unused_must_use)]

use futures::stream::StreamExt;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use sorock_core::*;
use std::path::Path;
use std::time::Duration;
mod fd_app_out_impl;

#[derive(serde::Deserialize, Debug)]
struct Config {
    #[serde(with = "http_serde::uri")]
    uri: tonic::transport::Uri,
    cap: byte_unit::Byte,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = envy::from_env::<Config>()?;
    let uri = config.uri;
    let cap = config.cap;

    let SOROCKDB_ROOT = Path::new("/var/lib/sorock/data");
    if SOROCKDB_ROOT.join("dead_flag").exists() {
        anyhow::bail!("This node can't not be restarted because if failed in the previous shutdown. Please clean the volume.");
    }
    std::fs::write(SOROCKDB_ROOT.join("dead_flag"), "")?;

    if !SOROCKDB_ROOT.join("init_flag").exists() {
        eprintln!("init_flag doesn't exist. The state will be recreated.");

        let snapshots = SOROCKDB_ROOT.join("snapshots");
        lol_core::simple::FileRepository::create(&snapshots)?;

        let raft_log = SOROCKDB_ROOT.join("raft_log");
        lol_core::storage::rocksdb::Storage::create(&raft_log)?;

        let piecedb = SOROCKDB_ROOT.join("piecedb");
        std::fs::create_dir(&piecedb)?;
        piece_store::sqlite::State::new(piece_store::sqlite::StoreType::Directory {
            root_dir: piecedb,
        })
        .await;

        std::fs::write(SOROCKDB_ROOT.join("init_flag"), "")?;
    }

    // Storage Service

    let peer_out_cli = peer_out::spawn(peer_out::State::new());
    let io_front_cli = io_front::spawn(peer_out_cli.clone(), io_front::State::new());
    // let piece_store_cli = mem_piece_store::spawn(mem_piece_store::State::new());
    let piece_store_cli = piece_store::sqlite::spawn(
        piece_store::sqlite::State::new(piece_store::sqlite::StoreType::Directory {
            root_dir: SOROCKDB_ROOT.join("piecedb"),
        })
        .await,
    );
    let stabilizer_cli = stabilizer::spawn(
        piece_store_cli.clone(),
        peer_out_cli.clone(),
        stabilizer::State::new(uri.clone()),
    );
    stabilizer::spawn_tick(stabilizer_cli.clone(), Duration::from_millis(100));
    let rebuild_queue_cli = rebuild_queue::spawn(
        piece_store_cli.clone(),
        peer_out_cli.clone(),
        stabilizer_cli.clone(),
        rebuild_queue::State::new(),
    );
    rebuild_queue::spawn_tick(rebuild_queue_cli.clone(), Duration::from_millis(500));
    let peer_in_cli = peer_in::spawn(
        piece_store_cli,
        stabilizer_cli.clone(),
        rebuild_queue_cli.clone(),
        peer_in::State::new(),
    );
    let cap = cap.get_adjusted_unit(byte_unit::ByteUnit::TiB);
    let server = storage_service::Server::new(
        io_front_cli.clone(),
        peer_in_cli.clone(),
        uri.clone(),
        cap.get_value(),
    );
    let svc1 = storage_service::make_service(server).await;

    // Failure Detector Service

    use failure_detector as FD;
    let peer_out_cli = FD::peer_out::spawn(FD::peer_out::State::new());
    let app_out_cli = fd_app_out_impl::spawn(fd_app_out_impl::State::new(uri.clone()));
    let queue_cli = FD::queue::spawn(peer_out_cli.clone(), app_out_cli, FD::queue::State::new());
    let reporter_cli =
        FD::reporter::spawn(queue_cli.clone(), FD::reporter::State::new(uri.clone()));
    let mut app_in_cli = FD::app_in::spawn(queue_cli.clone(), reporter_cli.clone());
    let svc2 = FD::server::make_service(FD::server::Server {
        peer_out_cli,
        uri: uri.clone(),
    })
    .await;
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let mut reporter_cli = reporter_cli.clone();
        let mut queue_cli = queue_cli.clone();
        loop {
            interval.tick().await;
            if reporter_cli.run_once().await.unwrap().is_err() {
                continue;
            }
            if queue_cli.run_once().await.unwrap().is_err() {
                continue;
            }
        }
    });

    // Raft Service

    let cluster_in_cli = cluster_in::spawn(
        io_front_cli,
        stabilizer_cli,
        peer_in_cli,
        rebuild_queue_cli,
        app_in_cli,
    );
    let raft_app = raft_service::App::new(cluster_in_cli);
    let raft_app = lol_core::simple::ToRaftApp::new(
        raft_app,
        lol_core::simple::FileRepository::open(&SOROCKDB_ROOT.join("snapshots"))?,
    );
    let config = lol_core::ConfigBuilder::default()
        .compaction_interval_sec(0)
        .build()
        .unwrap();
    let svc3 = lol_core::make_raft_service(
        raft_app,
        lol_core::storage::rocksdb::Storage::open(&SOROCKDB_ROOT.join("raft_log"))?,
        // lol_core::storage::memory::Storage::new(),
        uri.clone(),
        config,
    )
    .await;

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

    let mut builder = tonic::transport::Server::builder();
    let socket_addr = format!(
        "0.0.0.0:{port}",
        port = uri.port().expect("URI should have a port")
    );
    let socket = tokio::net::lookup_host(socket_addr)
        .await
        .unwrap()
        .next()
        .expect("couldn't resolve socket address.");

    builder
        .add_service(svc1)
        .add_service(svc2)
        .add_service(svc3)
        .serve_with_shutdown(socket, async {
            rx.await.ok();
        })
        .await
        .expect("couldn't start the server.");

    // On successful shutdown, the dead flag is removed.
    std::fs::remove_file(SOROCKDB_ROOT.join("dead_flag"))?;
    eprintln!("success in shutdown.");

    Ok(())
}
