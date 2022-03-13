use crate::*;
define_client!(PieceStore);
#[norpc::service]
trait PieceStore {
    fn get_piece(loc: PieceLocator) -> Option<Vec<u8>>;
    fn put_piece(loc: PieceLocator, data: Bytes);
    fn delete_piece(loc: PieceLocator);
    fn keys() -> Vec<String>;
}