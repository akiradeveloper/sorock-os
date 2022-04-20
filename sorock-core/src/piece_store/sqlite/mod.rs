use crate::*;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::ConnectOptions;
use sqlx::Executor;
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

pub fn spawn(state: State) -> piece_store::ClientT {
    use norpc::runtime::tokio::*;
    let svc = App { state };
    let svc = piece_store::PieceStoreService::new(svc);
    let (chan, server) = ServerBuilder::new(svc).build();
    tokio::spawn(server.serve());
    piece_store::PieceStoreClient::new(chan)
}

pub enum StoreType {
    Memory,
    // If the root_dir is empty then a new db is created.
    Directory { root_dir: PathBuf },
}
pub struct State {
    db_pool: SqlitePool,
}
impl State {
    pub async fn new(typ: StoreType) -> Self {
        let options = match typ {
            StoreType::Memory => {
                let mut options = SqliteConnectOptions::from_str(":memory:").unwrap();
                options
            }
            StoreType::Directory { root_dir } => {
                let mut path = root_dir;
                path.set_file_name("store.db");

                let mut options =
                    SqliteConnectOptions::from_str(&format!("sqlite://{}", path.to_str().unwrap()))
                        .unwrap()
                        .create_if_missing(true);
                options
            }
        };

        let db_pool = SqlitePool::connect_with(options).await.unwrap();
        let q = include_str!("./schema.sql");
        db_pool.execute(q).await.unwrap();

        Self { db_pool }
    }
}
#[derive(sqlx::FromRow, Debug)]
struct Rec {
    idx: i64,
    data: Vec<u8>,
}
#[derive(sqlx::FromRow, Debug)]
struct Key {
    key: String,
}
struct App {
    state: State,
}
#[norpc::async_trait]
impl piece_store::PieceStore for App {
    async fn get_pieces(&self, key: String, n: u8) -> anyhow::Result<Vec<(u8, Vec<u8>)>> {
        let q = "select idx, data from sorockdb where key = $1";
        let recs = sqlx::query_as::<_, Rec>(q)
            .bind(key)
            .fetch_all(&self.state.db_pool)
            .await?;
        let mut out = vec![];
        for Rec { idx, data, .. } in recs {
            out.push((idx as u8, data));
        }
        Ok(out)
    }
    async fn get_piece(&self, loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>> {
        let q = "select idx, data from sorockdb where key = $1 and idx = $2";
        let rec = sqlx::query_as::<_, Rec>(q)
            .bind(loc.key)
            .bind(loc.index)
            .fetch_optional(&self.state.db_pool)
            .await?;
        match rec {
            None => Ok(None),
            Some(rec) => Ok(Some(rec.data)),
        }
    }
    async fn piece_exists(&self, loc: PieceLocator) -> anyhow::Result<bool> {
        let q = "select count(*) from sorockdb where key = $1 and idx = $2";
        let rec: (i32,) = sqlx::query_as(q)
            .bind(loc.key)
            .bind(loc.index)
            .fetch_one(&self.state.db_pool)
            .await?;
        Ok(rec.0 > 0)
    }
    async fn put_piece(&self, loc: PieceLocator, data: Bytes) -> anyhow::Result<()> {
        let q = "insert into sorockdb (key, idx, data) values ($1, $2, $3)";
        sqlx::query(q)
            .bind(loc.key)
            .bind(loc.index)
            .bind(data.as_ref())
            .execute(&self.state.db_pool)
            .await?;
        Ok(())
    }
    async fn delete_piece(&self, loc: PieceLocator) -> anyhow::Result<()> {
        let q = "delete from sorockdb where key = $1 and idx = $2";
        sqlx::query(q)
            .bind(loc.key)
            .bind(loc.index)
            .execute(&self.state.db_pool)
            .await?;
        Ok(())
    }
    async fn keys(&self) -> anyhow::Result<Vec<String>> {
        let q = "select key from sorockdb";
        let keys = sqlx::query_as::<_, Key>(q)
            .fetch_all(&self.state.db_pool)
            .await?;
        let mut out = HashSet::new();
        for key in keys {
            out.insert(key.key);
        }
        let out = out.into_iter().collect();
        Ok(out)
    }
}

#[tokio::test]
async fn test_sqlite_store_mem() -> anyhow::Result<()> {
    let state = State::new(StoreType::Memory).await;
    let cli = spawn(state);
    piece_store::test_piece_store(cli).await
}

#[tokio::test]
async fn test_sqlite_store_dir() -> anyhow::Result<()> {
    let tempdir = tempfile::tempdir()?;
    let root_dir = tempdir.path().to_owned();
    let state = State::new(StoreType::Directory { root_dir }).await;
    let cli = spawn(state);
    piece_store::test_piece_store(cli).await
}
