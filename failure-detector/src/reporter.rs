use crate::*;

#[norpc::service]
trait Reporter {
    fn run_once();
    fn set_new_cluster();
}
define_client!(Reporter);
