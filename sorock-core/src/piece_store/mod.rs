use crate::*;

pub mod mem;
pub mod sqlite;

#[norpc::service]
trait PieceStore {
    fn get_pieces(key: String, n: u8) -> anyhow::Result<Vec<(u8, Vec<u8>)>>;
    fn get_piece(loc: PieceLocator) -> anyhow::Result<Option<Vec<u8>>>;
    fn put_piece(loc: PieceLocator, data: Bytes) -> anyhow::Result<()>;
    fn delete_piece(loc: PieceLocator) -> anyhow::Result<()>;
    fn piece_exists(loc: PieceLocator) -> anyhow::Result<bool>;
    fn keys() -> anyhow::Result<Vec<String>>;
}
define_client!(PieceStore);
