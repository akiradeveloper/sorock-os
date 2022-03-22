use crate::*;

#[norpc::service]
trait AppOut {
    // fn probe() -> bool;
    fn notify_failure(culprit: Uri);
}
define_client!(AppOut);
