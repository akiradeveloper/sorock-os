use crate::*;

#[norpc::service]
trait AppIn {
    fn report_suspect();
    fn set_new_cluster();
}
define_client!(AppIn);