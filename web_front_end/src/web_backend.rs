use core_lib::call_macro_with_request_list;
use serde::{Serialize, de::DeserializeOwned};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, channel};
use ui_lib::app_backend::AppBackend;

macro_rules! implement_requests {

    // For each request, redirect to one of the @func arms depending on the
    // request signature.
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        $(implement_requests!(@func $access $request ($($arg_ty)?) -> $ret_ty);)*
    };
    // Template for defining the start-request method for the unlock request. This request is special
    // in that a successful response contains the access token that the web-backend adapter must
    // store before any token-protected requests can be started.
    (@func Public unlock ($arg_ty:ty) -> $ret_ty:ty) => {
        fn start_unlock(&self, args: $arg_ty) -> Receiver<eyre::Result<core_lib::AccessGrant>> {
            self.start_unlock_request("unlock", args)
        }
    };
    // Template for defining the start-request method for a request without an argument.
    (@func $access:ident $request:ident () -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self) -> Receiver<eyre::Result<$ret_ty>> {
                self.start_request::<(), $ret_ty>(
                    AccessPolicy::$access,
                    stringify!($request),
                    (),
                    |_, _| {},
                )
            }
        }
    };
    // Template for defining the start-request method for a request with one argument.
    (@func $access:ident $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        paste::paste! {
            fn [<start_ $request>](&self, args: $arg_ty) -> Receiver<eyre::Result<$ret_ty>> {
                self.start_request::<$arg_ty, $ret_ty>(
                    AccessPolicy::$access,
                    stringify!($request),
                    args,
                    |_, _| {},
                )
            }
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AccessPolicy {
    Public,
    Token,
}

#[derive(Default)]
struct WebBackendState {
    access_token: Option<String>,
}

#[derive(Clone)]
pub struct WebBackend {
    state: Rc<RefCell<WebBackendState>>,
}

impl WebBackend {
    const LOCAL_BACKEND_URL: &'static str = "http://127.0.0.1:3000";
    const ACCESS_TOKEN_HEADER: &'static str = "x-tallytail-access-token";

    pub fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(WebBackendState::default())),
        }
    }

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

    async fn post<Args, Ret>(
        &self,
        access_policy: AccessPolicy,
        request: &str,
        args: Args,
    ) -> eyre::Result<Ret>
    where
        Args: Serialize,
        Ret: DeserializeOwned,
    {
        let url = Self::request_url(request);
        let mut request_builder = reqwest::Client::new().post(&url).json(&args);

        if access_policy == AccessPolicy::Token {
            let access_token = self
                .state
                .borrow()
                .access_token
                .clone()
                .ok_or_else(|| eyre::eyre!("Unlock required"))?;
            request_builder = request_builder.header(Self::ACCESS_TOKEN_HEADER, access_token);
        }

        let response = request_builder.send().await?;

        if !response.status().is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_else(|_| "Failed to read error response body".to_string());
            return Err(eyre::eyre!(message));
        }

        Ok(response.json::<Ret>().await?)
    }

    fn start_request<Args, Ret>(
        &self,
        access_policy: AccessPolicy,
        request: &'static str,
        args: Args,
        on_success: impl FnOnce(&Self, &Ret) + 'static,
    ) -> Receiver<eyre::Result<Ret>>
    where
        Args: Serialize + 'static,
        Ret: DeserializeOwned + 'static,
    {
        let (tx, rx) = channel();
        let backend = self.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = backend
                .post::<Args, Ret>(access_policy, request, args)
                .await;
            if let Ok(value) = &result {
                on_success(&backend, value);
            }
            let _ = tx.send(result);
        });

        rx
    }

    fn start_unlock_request<Args>(
        &self,
        request: &'static str,
        args: Args,
    ) -> Receiver<eyre::Result<core_lib::AccessGrant>>
    where
        Args: Serialize + 'static,
    {
        self.start_request(
            AccessPolicy::Public,
            request,
            args,
            |backend, access_grant: &core_lib::AccessGrant| {
                backend.state.borrow_mut().access_token = Some(access_grant.access_token.clone());
            },
        )
    }
}

impl AppBackend for WebBackend {
    call_macro_with_request_list!(implement_requests);
}
