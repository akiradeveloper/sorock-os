use crate::*;
use std::collections::HashSet;

#[norpc::service]
trait AppIn {
    fn report_suspect();
    fn set_new_cluster(cluster: HashSet<Uri>);
}
define_client!(AppIn);
