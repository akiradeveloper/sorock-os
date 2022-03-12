use crate::*;

use bytes::Bytes;
use tonic::transport::Uri;

#[norpc::service]
trait IOFront {
    fn create(key: String, value: Bytes);
    fn read(key: String) -> Bytes;
    fn set_new_cluster(cluster: ClusterMap);
}

#[norpc::service]
trait Stabilizer {
    fn set_new_cluster(cluster: ClusterMap);
}

#[norpc::service]
trait PieceStore {
    fn get_piece(loc: PieceLocator) -> Option<Vec<u8>>;
    fn put_piece(loc: PieceLocator, data: Bytes);
    fn delete_piece(loc: PieceLocator);
    fn keys() -> Vec<String>;
}

#[norpc::service]
trait PeerIn {
    fn set_new_cluster(cluster: ClusterMap);
    fn save_piece(piece: SendPiece);
    fn find_piece(loc: PieceLocator) -> Option<Vec<u8>>;
    fn find_any_pieces(key: String) -> Vec<(u8, Vec<u8>)>;
}

#[norpc::service]
trait PeerOut {
    fn send_piece(to: Uri, piece: SendPiece);
    fn request_piece(to: Uri, loc: PieceLocator) -> Option<Vec<u8>>;
    fn request_any_pieces(to: Uri, key: String) -> Vec<(u8, Vec<u8>)>;
}

#[norpc::service]
trait ClusterIn {
    fn set_new_cluster(cluster: ClusterMap);
}
