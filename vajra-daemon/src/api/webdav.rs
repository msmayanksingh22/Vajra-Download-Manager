use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    response::Response,
};
use dav_server::DavHandler;

pub async fn webdav_handler(State(handler): State<Arc<DavHandler>>, req: Request) -> Response {
    // Forward the request to the dav-server handler
    let res = handler.handle(req).await;
    let (parts, body) = res.into_parts();
    Response::from_parts(parts, Body::new(body))
}
