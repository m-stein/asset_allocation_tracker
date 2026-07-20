use core_lib::with_gui_requests;
use std::sync::mpsc::Receiver;

macro_rules! define_app_backend {
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        paste::paste! {
            pub trait AppBackend {
                $(define_app_backend!(@method $request ($($arg_ty)?) -> $ret_ty);)*
            }
        }
    };
    (@method $request:ident () -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self) -> Receiver<eyre::Result<$ret_ty>>;
        }
    };
    (@method $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self, args: $arg_ty) -> Receiver<eyre::Result<$ret_ty>>;
        }
    };
}

with_gui_requests!(define_app_backend);
