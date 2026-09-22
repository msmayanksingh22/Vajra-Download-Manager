use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use dav_server::{localfs::LocalFs, DavHandler};

use crate::AppState;

pub async fn webdav_handler(State(state): State<Arc<AppState>>, req: Request) -> Response {
    let (enabled, read_only, out_dir) = {
        let cfg = state.config.read().await;
        (
            cfg.webdav_enabled,
            cfg.webdav_read_only,
            cfg.default_output_dir.clone(),
        )
    };

    if !enabled {
        return StatusCode::NOT_FOUND.into_response();
    }

    let dav_root = match vajra_protocol::path_security::get_controlled_webdav_root(
        std::path::Path::new(&out_dir),
    ) {
        Ok(p) => {
            let _ = std::fs::create_dir_all(&p);
            p
        }
        Err(e) => {
            tracing::error!("Invalid WebDAV root directory: {e}");
            return StatusCode::FORBIDDEN.into_response();
        }
    };

    let dav_server = DavHandler::builder()
        .filesystem(LocalFs::new(dav_root, false, read_only, false))
        .locksystem(dav_server::memls::MemLs::new())
        .build_handler();

    let res = dav_server.handle(req).await;
    let (parts, body) = res.into_parts();
    Response::from_parts(parts, Body::new(body))
}

#[cfg(test)]
mod tests {
    use axum::http::Request;

    use super::*;

    #[tokio::test]
    async fn test_dav_server_request() {
        let temp = tempfile::tempdir().unwrap();
        let dav_server = DavHandler::builder()
            .filesystem(LocalFs::new(temp.path(), false, true, false))
            .locksystem(dav_server::memls::MemLs::new())
            .build_handler();

        let req = Request::builder()
            .uri("/test.txt")
            .header("host", "127.0.0.1:16277")
            .body(Body::empty())
            .unwrap();

        let res = dav_server.handle(req).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }
}
