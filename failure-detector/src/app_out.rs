use crate::*;

#[norpc::service]
trait AppOut {
    fn probe() -> bool;
    fn notify_failure();
}
define_client!(AppOut);