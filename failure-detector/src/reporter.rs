use crate::*;
use std::collections::HashSet;

#[norpc::service]
trait Reporter {
    fn run_once();
    fn set_new_cluster(cluster: HashSet<Uri>);
}
define_client!(Reporter);
