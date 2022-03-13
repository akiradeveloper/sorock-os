use sorock::*;
use signal_hook::consts::signal::*;
use signal_hook_tokio::Signals;
use futures::stream::StreamExt;

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
