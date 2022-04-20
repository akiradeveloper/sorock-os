#![deny(unused_must_use)]

//! Failure detector based on "On Scalable and Efficient Distributed Failure Detectors" (Gupta et al.)

#[macro_export]
macro_rules! define_client {
    ($name: ident) => {
        paste::paste! {
            pub type ClientT = [<$name Client>]<norpc::runtime::tokio::Channel<[<$name Request>], [<$name Response>] >>;
        }
    };
}

use tokio::sync::RwLock;
use tonic::transport::Uri;

pub mod app_in;
pub mod app_out;
pub mod peer_out;
pub mod queue;
pub mod reporter;
pub mod server;

pub mod proto_compiled {
    tonic::include_proto!("fd");
}
