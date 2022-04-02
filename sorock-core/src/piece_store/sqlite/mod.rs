use crate::*;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePool;
use sqlx::ConnectOptions;
use sqlx::Executor;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

pub fn spawn(state: State) -> piece_store::ClientT {
    use norpc::runtime::send::*;
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    tokio::spawn(async {
        let svc = App {
            state: state.into(),
        };
        let service = piece_store::PieceStoreService::new(svc);
        let server = ServerExecutor::new(rx, service);
        server.serve().await
    });
    let chan = ClientService::new(tx);
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
    index: i64,
    data: Vec<u8>,
}
#[derive(sqlx::FromRow, Debug)]
struct Key {
    key: String,
}
#[derive(Clone)]
struct App {
    state: Arc<State>,
}
#[norpc::async_trait]
impl piece_store::PieceStore for App {
    async fn get_pieces(self, key: String, n: u8) -> anyhow::Result<Vec<(u8, Vec<u8>)>> {
        let q = "select index, data from sorockdb where key = $1";
        let recs = sqlx::query_as::<_, Rec>(q)
            .bind(key)
            .fetch_all(&self.state.db_pool)
            .await?;
        let mut out = vec![];
        for Rec { index, data, .. } in recs {
            out.push((index as u8, data));
        }
        Ok(out)
    }
    async fn get_piece(self, loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>> {
        let q = "select index, data from sorockdb where key = $1 and index = $2";
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
    async fn piece_exists(self, loc: PieceLocator) -> anyhow::Result<bool> {
        let q = "select count(*) from sorockdb where key = $1 and index = $2";
        let rec: (i32,) = sqlx::query_as(q)
            .bind(loc.key)
            .bind(loc.index)
            .fetch_one(&self.state.db_pool)
            .await?;
        Ok(rec.0 > 0)
    }
    async fn put_piece(self, loc: PieceLocator, data: Bytes) -> anyhow::Result<()> {
        let q = "insert into sorockdb (key, index, data) values ($1, $2, $3)";
        sqlx::query(q)
            .bind(loc.key)
            .bind(loc.index)
            .bind(data.as_ref())
            .execute(&self.state.db_pool)
            .await?;
        Ok(())
    }
    async fn delete_piece(self, loc: PieceLocator) -> anyhow::Result<()> {
        let q = "delete from sorockdb where key = $1 and index = $2";
        sqlx::query(q)
            .bind(loc.key)
            .bind(loc.index)
            .execute(&self.state.db_pool)
            .await?;
        Ok(())
    }
    async fn keys(self) -> anyhow::Result<Vec<String>> {
        let q = "select key from sorockdb";
        let keys = sqlx::query_as::<_, Key>(q)
            .fetch_all(&self.state.db_pool)
            .await?;
        let mut out = vec![];
        for key in keys {
            out.push(key.key);
        }
        Ok(out)
    }
}

#[tokio::test]
async fn test_sqlite_store() -> anyhow::Result<()> {
    let state = State::new(StoreType::Memory).await;
    let mut cli = spawn(state);
    assert_eq!(cli.keys().await??.len(), 0);
    assert_eq!(cli.get_pieces("a".to_string(), 8).await??, vec![]);
    assert_eq!(
        cli.piece_exists(PieceLocator {
            key: "a".to_string(),
            index: 1
        })
        .await??,
        false
    );
    Ok(())
}
