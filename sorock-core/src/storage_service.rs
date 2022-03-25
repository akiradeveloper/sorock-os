use crate::*;

mod proto_compiled {
    tonic::include_proto!("sorock");
}
use proto_compiled::{
    sorock_server::Sorock, AddNodeReq, CreateReq, DeleteReq, IndexedPiece, PieceExistsRep,
    PieceExistsReq, ReadRep, ReadReq, RemoveNodeReq, RequestAnyPiecesRep, RequestAnyPiecesReq,
    RequestPieceRep, RequestPieceReq, SanityCheckRep, SanityCheckReq, SendPieceRep, SendPieceReq,
};

pub struct Server {
    io_front_cli: io_front::ClientT,
    peer_in_cli: peer_in::ClientT,
    self_chan: tonic::transport::Channel,
}
impl Server {
    pub fn new(io_front_cli: io_front::ClientT, peer_in_cli: peer_in::ClientT, uri: Uri) -> Self {
        let e = tonic::transport::Endpoint::new(uri).unwrap();
        let self_chan = e.connect_lazy();
        Self {
            io_front_cli,
            peer_in_cli,
            self_chan,
        }
    }
}
#[tonic::async_trait]
impl Sorock for Server {
    async fn read(
        &self,
        req: tonic::Request<ReadReq>,
    ) -> Result<tonic::Response<ReadRep>, tonic::Status> {
        let req = req.into_inner();
        let mut cli = self.io_front_cli.clone();
        let key = req.key;
        let res = cli.read(key).await.unwrap().unwrap();
        let rep = ReadRep { data: res };
        Ok(tonic::Response::new(rep))
    }
    async fn sanity_check(
        &self,
        req: tonic::Request<SanityCheckReq>,
    ) -> Result<tonic::Response<SanityCheckRep>, tonic::Status> {
        let req = req.into_inner();
        let mut cli = self.io_front_cli.clone();
        let key = req.key;
        let n_lost = cli.sanity_check(key).await.unwrap().unwrap();
        let rep = SanityCheckRep {
            n_lost: n_lost as u32,
        };
        Ok(tonic::Response::new(rep))
    }
    async fn create(
        &self,
        request: tonic::Request<CreateReq>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let req = request.into_inner();
        let mut cli = self.io_front_cli.clone();
        let key = req.key;
        let data = req.data;
        cli.create(key, data).await.unwrap().unwrap();
        Ok(tonic::Response::new(()))
    }
    async fn delete(
        &self,
        request: tonic::Request<DeleteReq>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        unimplemented!()
    }
    async fn ping(
        &self,
        request: tonic::Request<()>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        Ok(tonic::Response::new(()))
    }
    async fn add_node(
        &self,
        request: tonic::Request<AddNodeReq>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let req = request.into_inner();
        let chan = self.self_chan.clone();
        let mut cli = lol_core::RaftClient::new(chan);
        let tgt_uri: Uri = req.uri.parse().unwrap();
        let msg = Command::AddNode {
            uri: URI(tgt_uri),
            cap: req.cap,
        };
        cli.request_commit(lol_core::api::CommitReq {
            message: Command::encode(&msg),
        })
        .await
        .unwrap();
        Ok(tonic::Response::new(()))
    }
    async fn remove_node(
        &self,
        request: tonic::Request<RemoveNodeReq>,
    ) -> Result<tonic::Response<()>, tonic::Status> {
        let req = request.into_inner();
        let chan = self.self_chan.clone();
        let mut cli = lol_core::RaftClient::new(chan);
        let tgt_uri = req.uri.parse().unwrap();
        let msg = Command::RemoveNode { uri: URI(tgt_uri) };
        cli.request_commit(lol_core::api::CommitReq {
            message: Command::encode(&msg),
        })
        .await
        .unwrap();
        Ok(tonic::Response::new(()))
    }
    async fn piece_exists(
        &self,
        req: tonic::Request<PieceExistsReq>,
    ) -> Result<tonic::Response<PieceExistsRep>, tonic::Status> {
        let req = req.into_inner();
        let loc = PieceLocator {
            key: req.key,
            index: req.index as u8,
        };
        let mut cli = self.peer_in_cli.clone();
        let rep = cli.piece_exists(loc).await.unwrap().unwrap();
        Ok(tonic::Response::new(PieceExistsRep { exists: rep }))
    }
    async fn send_piece(
        &self,
        request: tonic::Request<SendPieceReq>,
    ) -> Result<tonic::Response<SendPieceRep>, tonic::Status> {
        let req = request.into_inner();
        let mut cli = self.peer_in_cli.clone();
        let send_piece = SendPiece {
            version: req.version,
            loc: PieceLocator {
                key: req.key,
                index: req.index as u8,
            },
            data: req.data,
        };
        let rep = cli.save_piece(send_piece).await.unwrap();
        let error_code = match rep {
            Ok(()) => 0,
            Err(SendPieceError::Rejected) => -1,
            Err(SendPieceError::Failed) => -2,
        };
        Ok(tonic::Response::new(SendPieceRep { error_code }))
    }
    async fn request_piece(
        &self,
        request: tonic::Request<RequestPieceReq>,
    ) -> Result<tonic::Response<RequestPieceRep>, tonic::Status> {
        let req = request.into_inner();
        let mut cli = self.peer_in_cli.clone();
        let loc = PieceLocator {
            key: req.key,
            index: req.index as u8,
        };
        let res = cli.find_piece(loc).await.unwrap().unwrap();
        let rep = RequestPieceRep { data: res };
        Ok(tonic::Response::new(rep))
    }
    async fn request_any_pieces(
        &self,
        req: tonic::Request<RequestAnyPiecesReq>,
    ) -> Result<tonic::Response<RequestAnyPiecesRep>, tonic::Status> {
        let mut cli = self.peer_in_cli.clone();
        let req = req.into_inner();
        let key = req.key;
        let rep = cli.find_any_pieces(key).await.unwrap().unwrap();
        let mut pieces = vec![];
        for (i, data) in rep {
            pieces.push(IndexedPiece {
                index: i as u32,
                data,
            });
        }
        let out = RequestAnyPiecesRep { pieces };
        Ok(tonic::Response::new(out))
    }
}

pub async fn make_service(server: Server) -> proto_compiled::sorock_server::SorockServer<Server> {
    proto_compiled::sorock_server::SorockServer::new(server)
}
