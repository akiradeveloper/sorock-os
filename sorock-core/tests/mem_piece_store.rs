use sorock::*;
use std::collections::HashMap;
use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn spawn(state: State) -> sorock::piece_store::ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            state: Arc::new(state),
        };
        let service = sorock::piece_store::PieceStoreService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    sorock::piece_store::PieceStoreClient::new(chan)
}

#[tokio::test]
async fn test_piece_store() {
    let mut cli = spawn(State::new());
    let loc = PieceLocator {
        key: "ABC".to_string(),
        index: 3,
    };
    let data = Bytes::copy_from_slice(&[1, 2, 3]);
    assert_eq!(cli.get_piece(loc.clone()).await.unwrap(), None);
    cli.put_piece(loc.clone(), data).await.unwrap();
    assert_eq!(
        cli.get_piece(loc.clone()).await.unwrap(),
        Some(vec![1, 2, 3])
    );
}

struct Bucket {
    objects: Vec<Option<Bytes>>,
}
impl Bucket {
    fn new() -> Self {
        Self {
            objects: vec![None; N],
        }
    }
}

pub struct State {
    buckets: RwLock<HashMap<String, Bucket>>,
}
impl State {
    pub fn new() -> Self {
        Self {
            buckets: RwLock::new(HashMap::new()),
        }
    }
    async fn get_piece(&self, loc: PieceLocator) -> Option<Bytes> {
        let buckets = self.buckets.read().await;
        let bucket = buckets.get(&loc.key);
        match bucket {
            Some(bucket) => bucket.objects[loc.index as usize].clone(),
            None => None,
        }
    }
    async fn put_piece(&self, loc: PieceLocator, data: Bytes) {
        let mut buckets = self.buckets.write().await;
        let bucket = buckets.entry(loc.key).or_insert(Bucket::new());
        bucket.objects[loc.index as usize] = Some(data);
    }
    async fn delete_piece(&self, loc: PieceLocator) {
        let mut buckets = self.buckets.write().await;
        let bucket = buckets.entry(loc.key.clone()).or_insert(Bucket::new());
        bucket.objects[loc.index as usize] = None;

        // If the bucket doesn't have anything in it remove it.
        let mut all_none = true;
        for i in 0..N {
            if bucket.objects[i].is_some() {
                all_none = false;
            }
        }
        if all_none {
            buckets.remove(&loc.key);
        }
    }
    async fn keys(&self) -> Vec<String> {
        let buckets = self.buckets.read().await;
        let mut out = vec![];
        for (k, _) in buckets.iter() {
            out.push(k.clone());
        }
        out
    }
}

#[derive(Clone)]
struct App {
    state: Arc<State>,
}
#[norpc::async_trait]
impl piece_store::PieceStore for App {
    async fn get_piece(self, loc: PieceLocator) -> Option<Vec<u8>> {
        let buf = self.state.get_piece(loc).await;
        buf.map(|buf| {
            let mut out = vec![];
            out.extend_from_slice(&buf);
            out
        })
    }
    async fn put_piece(self, loc: PieceLocator, data: Bytes) {
        self.state.put_piece(loc, data).await
    }
    async fn delete_piece(self, loc: PieceLocator) {
        self.state.delete_piece(loc).await
    }
    async fn keys(self) -> Vec<String> {
        self.state.keys().await
    }
}
