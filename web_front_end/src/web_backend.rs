use core_lib::call_macro_with_request_list;
use serde::{Serialize, de::DeserializeOwned};
use std::sync::mpsc::{Receiver, channel};
use ui_lib::app_backend::AppBackend;

macro_rules! implement_requests {

    // For each request, redirect to one of the @func arms depending on whether
    // the request has an argument or not
    ($($request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        $(implement_requests!(@func $request ($($arg_ty)?) -> $ret_ty);)*
    };
    // Start-request function-template for requests without an argument
    (@func $request:ident () -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self) -> Receiver<eyre::Result<$ret_ty>> {
                Self::start_request::<(), $ret_ty>(stringify!($request), ())
            }
        }
    };
    // Start-request function-template for requests with one argument
    (@func $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self, args: $arg_ty) -> Receiver<eyre::Result<$ret_ty>> {
                Self::start_request::<$arg_ty, $ret_ty>(stringify!($request), args)
            }
        }
    };
}

pub struct WebBackend;

impl WebBackend {
    const LOCAL_BACKEND_URL: &'static str = "http://127.0.0.1:3000";

    fn request_url(request: &str) -> String {
        let window = web_sys::window().expect("missing window");
        let location = window.location();
        let hostname = location.hostname().unwrap_or_default();
        let port = location.port().unwrap_or_default();

        if (hostname == "127.0.0.1" || hostname == "localhost") && port != "3000" {
            format!("{}/{}", Self::LOCAL_BACKEND_URL, request)
        } else {
            let origin = location.origin().unwrap_or_default();
            format!("{origin}/{request}")
        }
    }

    async fn post<Args, Ret>(request: &str, args: Args) -> eyre::Result<Ret>
    where
        Args: Serialize,
        Ret: DeserializeOwned,
    {
        let url = Self::request_url(request);
        let response = reqwest::Client::new().post(&url).json(&args).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error response body".to_string());
            return Err(eyre::eyre!("{status} for {url}: {message}"));
        }

        Ok(response.json::<Ret>().await?)
    }

    fn start_request<Args, Ret>(request: &'static str, args: Args) -> Receiver<eyre::Result<Ret>>
    where
        Args: Serialize + 'static,
        Ret: DeserializeOwned + 'static,
    {
        let (tx, rx) = channel();
        wasm_bindgen_futures::spawn_local(async move {
            let result = Self::post::<Args, Ret>(request, args).await;
            let _ = tx.send(result);
        });

        rx
    }
}

impl AppBackend for WebBackend {
    call_macro_with_request_list!(implement_requests);
}
