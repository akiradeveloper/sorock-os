use std::future::Future;
use tokio::task::JoinError;

pub fn into_safe_future<T: Send + 'static>(
    fut: impl Future<Output = T> + Send + 'static,
) -> impl Future<Output = Result<T, JoinError>> {
    async move { tokio::spawn(fut).await }
}

#[tokio::test(flavor = "multi_thread")]
async fn test_safe_future() {
    use futures::stream::StreamExt;
    use futures::FutureExt;
    use std::time::Duration;
    use std::time::Instant;
    let mut futs = vec![];
    futs.push(
        into_safe_future(async move {
            panic!();
            1000
        })
        .boxed(),
    );
    for _ in 0..100 {
        futs.push(
            into_safe_future(async move {
                tokio::time::sleep(Duration::from_secs(1)).await;
                1
            })
            .boxed(),
        );
    }
    let stream = futures::stream::iter(futs);
    let mut buffered = stream.buffer_unordered(50);
    let t = Instant::now();
    let mut sum = 0;
    while let Some(rep) = buffered.next().await {
        if let Ok(x) = rep {
            sum += x;
        } else {
            dbg!("error");
        }
    }
    dbg!(t.elapsed());
    assert_eq!(sum, 100);
}
