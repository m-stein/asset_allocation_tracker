use core_lib::call_macro_with_request_list;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use ui_lib::app_backend::AppBackend;

macro_rules! implement_requests {

    // For each request, redirect to one of the @one arms depending on whether
    // the request has an argument or not
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        paste::paste! {
            $(implement_requests!(@handler $request ($($arg_ty)?) -> $ret_ty);)*
        }
    };
    (@handler unlock (core_lib::UnlockPatternInput) -> core_lib::AccessGrant) => {
        fn start_unlock(
            &self,
            _args: core_lib::UnlockPatternInput,
        ) -> Receiver<eyre::Result<core_lib::AccessGrant>> {
            let (tx, rx) = channel();
            thread::spawn(move || {
                let _ = tx.send(Err(eyre::eyre!(
                    "Unlock is only available in the web app"
                )));
            });
            rx
        }
    };
    // Request handler template for requests without arguments
    (@handler $request:ident () -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self) -> Receiver<eyre::Result<$ret_ty>> {
                let (tx, rx) = channel();
                thread::spawn(move || {
                    let _ = tx.send(infra_lib::$request());
                });
                rx
            }
        }
    };
    // Request handler template for requests with one argument
    (@handler $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self, args: $arg_ty) -> Receiver<eyre::Result<$ret_ty>> {
                let (tx, rx) = channel();
                thread::spawn(move || {
                    let _ = tx.send(infra_lib::$request(args));
                });
                rx
            }
        }
    };
}

pub struct DesktopBackend;

impl AppBackend for DesktopBackend {
    call_macro_with_request_list!(implement_requests);
}
