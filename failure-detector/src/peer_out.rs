use crate::*;
use tonic::transport::Uri;

#[norpc::service]
trait PeerOut {
    fn ping1(uri: Uri);
    fn ping2(uri: Uri);
}
define_client!(PeerOut);
