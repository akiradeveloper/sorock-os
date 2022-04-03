use crate::*;

use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub fn spawn(state: State) -> piece_store::ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            state: Arc::new(state),
        };
        let service = piece_store::PieceStoreService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
    piece_store::PieceStoreClient::new(chan)
}

#[tokio::test]
async fn test_piece_store_hashmap() -> anyhow::Result<()> {
    let mut cli = spawn(State::new());
    piece_store::test_piece_store(cli).await
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
    async fn piece_exists(&self, loc: PieceLocator) -> bool {
        let buckets = self.buckets.read().await;
        let bucket = buckets.get(&loc.key);
        match bucket {
            Some(bucket) => bucket.objects[loc.index as usize].is_some(),
            None => false,
        }
    }
    async fn get_pieces(&self, key: String, n: u8) -> Vec<(u8, Bytes)> {
        let buckets = self.buckets.read().await;
        let bucket = buckets.get(&key);
        match bucket {
            None => vec![],
            Some(bucket) => {
                let pieces = &bucket.objects;
                let mut out = vec![];
                for i in 0..n {
                    if let Some(piece) = &pieces[i as usize] {
                        out.push((i, piece.clone()))
                    }
                }
                out
            }
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
    async fn get_pieces(self, key: String, n: u8) -> anyhow::Result<Vec<(u8, Vec<u8>)>> {
        let pieces = self.state.get_pieces(key, n).await;
        let mut out = vec![];
        for (i, data) in pieces {
            let mut buf = vec![];
            buf.extend_from_slice(&data);
            out.push((i, buf));
        }
        Ok(out)
    }
    async fn get_piece(self, loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>> {
        let buf = self.state.get_piece(loc).await;
        let buf = buf.map(|buf| {
            let mut out = vec![];
            out.extend_from_slice(&buf);
            out
        });
        Ok(buf)
    }
    async fn piece_exists(self, loc: PieceLocator) -> anyhow::Result<bool> {
        Ok(self.state.piece_exists(loc).await)
    }
    async fn put_piece(self, loc: PieceLocator, data: Bytes) -> anyhow::Result<()> {
        self.state.put_piece(loc, data).await;
        Ok(())
    }
    async fn delete_piece(self, loc: PieceLocator) -> anyhow::Result<()> {
        self.state.delete_piece(loc).await;
        Ok(())
    }
    async fn keys(self) -> anyhow::Result<Vec<String>> {
        let out = self.state.keys().await;
        Ok(out)
    }
}
