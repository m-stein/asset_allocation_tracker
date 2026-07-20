mod access_control;
mod error;

use crate::access_control::{AccessControl, UnlockAttemptResult, ensure_has_access};
use crate::error::WebBackEndError;
use axum::{
    Json, Router,
    extract::State,
    http::HeaderMap,
    response::Html,
    routing::{get, get_service, post},
};
use core_lib::{with_gui_requests, with_nogui_requests};
use std::net::SocketAddr;
use std::path::PathBuf;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

macro_rules! implement_request_handlers {
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        $(implement_request_handlers!(@handler $access $request ($($arg_ty)?) -> $ret_ty);)*
    };
    (@handler Public unlock ($arg_ty:ty) -> $ret_ty:ty) => {
        async fn unlock(
            State(access_control): State<AccessControl>,
            headers: HeaderMap,
            Json(input): Json<$arg_ty>,
        ) -> Result<Json<$ret_ty>, WebBackEndError> {
            match access_control.unlock(&headers, &input.pattern)? {
                UnlockAttemptResult::Unlocked(session) => Ok(Json(session)),
                UnlockAttemptResult::PatternNotAccepted => {
                    Err(WebBackEndError::unauthorized("Pattern not accepted"))
                }
                UnlockAttemptResult::TooManyAttempts => Err(WebBackEndError::blocked(
                    "Too many attempts, access blocked",
                )),
            }
        }
    };
    (@handler Token $request:ident () -> $ret_ty:ty) => {
        async fn $request(
            State(access_control): State<AccessControl>,
            headers: HeaderMap,
            Json(()): Json<()>,
        ) -> Result<Json<$ret_ty>, WebBackEndError> {
            ensure_has_access(&access_control, &headers)?;
            Ok(Json(infra_lib::$request()?))
        }
    };
    (@handler Token $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        async fn $request(
            State(access_control): State<AccessControl>,
            headers: HeaderMap,
            Json(args): Json<$arg_ty>,
        ) -> Result<Json<$ret_ty>, WebBackEndError> {
            ensure_has_access(&access_control, &headers)?;
            Ok(Json(infra_lib::$request(args)?))
        }
    };
    (@handler Public $request:ident () -> $ret_ty:ty) => {
        async fn $request(Json(()): Json<()>) -> Result<Json<$ret_ty>, WebBackEndError> {
            Ok(Json(infra_lib::$request()?))
        }
    };
    (@handler Public $request:ident ($arg_ty:ty) -> $ret_ty:ty) => {
        async fn $request(Json(args): Json<$arg_ty>) -> Result<Json<$ret_ty>, WebBackEndError> {
            Ok(Json(infra_lib::$request(args)?))
        }
    };
}

macro_rules! create_request_router {
    ($(#[access($access:ident)] $request:ident($($arg_ty:ty)?) -> $ret_ty:ty;)*) => {
        Router::new()
            $(.route(concat!("/", stringify!($request)), post($request)))*
    };
}

fn build_router() -> Router<AccessControl> {
    let static_service = get_service(ServeDir::new("web_front_end/dist"));

    Router::new()
        .merge(with_gui_requests!(create_request_router))
        .merge(with_nogui_requests!(create_request_router))
        .route("/", get(index))
        .fallback_service(static_service)
}

async fn index() -> Html<String> {
    Html(std::fs::read_to_string("web_front_end/dist/index.html").unwrap())
}

with_gui_requests!(implement_request_handlers);
with_nogui_requests!(implement_request_handlers);

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let unlock_pattern = std::env::var("TALLYTAIL_UNLOCK_PATTERN")
        .map_err(|_| eyre::eyre!("TALLYTAIL_UNLOCK_PATTERN is required for web access"))?;
    let data_dir = std::env::var("TALLYTAIL_DATA_DIR")
        .map_err(|_| eyre::eyre!("TALLYTAIL_DATA_DIR is required for web access state"))?;
    let access_control = AccessControl::new(unlock_pattern, PathBuf::from(data_dir))?;
    let router = build_router()
        .with_state(access_control)
        .layer(CorsLayer::permissive());
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse()
        .unwrap_or(3000);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("Web back end runs on http://{addr}");
    axum::serve(listener, router).await?;
    Ok(())
}
