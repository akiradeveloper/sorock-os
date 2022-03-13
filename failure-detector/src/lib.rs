#[macro_export]
macro_rules! define_client {
    ($name: ident) => {
        paste::paste! {
            pub type ClientT = [<$name Client>]<norpc::runtime::send::ClientService<[<$name Request>], [<$name Response>] >>;
        }
    };
}

pub mod app_in;
pub mod app_out;