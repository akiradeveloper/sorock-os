use crate::*;

#[norpc::service]
trait Queue {
    fn queue_suspect();
    fn set_new_cluster();
	fn run_once();
}
define_client!(Queue);
