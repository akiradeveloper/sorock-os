use crate::*;
use tonic::transport::Uri;

#[norpc::service]
trait PeerOut {
    fn ping1(tgt: Uri);
    fn ping2(proxy: Uri, tgt: Uri);
}
define_client!(PeerOut);
