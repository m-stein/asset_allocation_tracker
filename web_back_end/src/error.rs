use axum::http::StatusCode;

pub struct WebBackEndError {
    status: StatusCode,
    error: eyre::Report,
}

impl<T> From<T> for WebBackEndError
where
    T: Into<eyre::Report>,
{
    fn from(error: T) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            error: error.into(),
        }
    }
}

impl WebBackEndError {
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            error: eyre::eyre!(message.into()),
        }
    }

    pub fn blocked(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::LOCKED,
            error: eyre::eyre!(message.into()),
        }
    }
}

impl axum::response::IntoResponse for WebBackEndError {
    fn into_response(self) -> axum::response::Response {
        (self.status, self.error.to_string()).into_response()
    }
}
