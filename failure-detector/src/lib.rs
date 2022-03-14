//! Failure detector based on "On Scalable and Efficient Distributed Failure Detectors" (Gupta et al.)
//! http://www.cs.cornell.edu/projects/Quicksilver/public_pdfs/On%20Scalable.pdf

#[macro_export]
macro_rules! define_client {
    ($name: ident) => {
        paste::paste! {
            pub type ClientT = [<$name Client>]<norpc::runtime::send::ClientService<[<$name Request>], [<$name Response>] >>;
        }
    };
}

use tonic::transport::Uri;

pub mod app_in;
pub mod app_out;
mod peer_out;
mod queue;
mod reporter;
mod server;