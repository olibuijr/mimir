//! The embedded web UI (built by `npm run build` in web/, copied in by build.rs) and cookie login.
use crate::server::{ct_eq, App, E};
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::sync::Arc;

const INDEX: &str = include_str!(concat!(env!("OUT_DIR"), "/index.html"));
const JS: &str = include_str!(concat!(env!("OUT_DIR"), "/app.js"));
const CSS: &str = include_str!(concat!(env!("OUT_DIR"), "/app.css"));
const ETAG: &str = concat!("\"", env!("MIMIR_ASSET_HASH"), "\"");

fn asset(h: &HeaderMap, ctype: &'static str, body: &'static str) -> Response {
    let mut r = if h.get(header::IF_NONE_MATCH).and_then(|v| v.to_str().ok()) == Some(ETAG) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        body.into_response()
    };
    let ok = r.status() == StatusCode::OK;
    let hs = r.headers_mut();
    if ok {
        hs.insert(header::CONTENT_TYPE, HeaderValue::from_static(ctype));
    }
    hs.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    hs.insert(header::ETAG, HeaderValue::from_static(ETAG));
    r
}

#[derive(Deserialize)]
struct Login {
    token: String,
}

async fn login(State(app): State<Arc<App>>, Json(l): Json<Login>) -> Result<Response, E> {
    if !ct_eq(&l.token, &app.token) {
        return Err(E(StatusCode::UNAUTHORIZED, "wrong token".into()));
    }
    // The token comes from MIMIR_TOKEN; refuse anything that would break the header.
    let c = format!("mimir={}; HttpOnly; SameSite=Strict; Path=/; Max-Age=2592000", app.token);
    let v = HeaderValue::from_str(&c).map_err(|_| E(StatusCode::INTERNAL_SERVER_ERROR, "token is not cookie-safe".into()))?;
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, v)]).into_response())
}

async fn logout() -> Response {
    (StatusCode::NO_CONTENT, [(header::SET_COOKIE, "mimir=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")]).into_response()
}

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/", get(|h: HeaderMap| async move { asset(&h, "text/html; charset=utf-8", INDEX) }))
        .route("/app.js", get(|h: HeaderMap| async move { asset(&h, "text/javascript; charset=utf-8", JS) }))
        .route("/app.css", get(|h: HeaderMap| async move { asset(&h, "text/css; charset=utf-8", CSS) }))
        .route("/api/login", post(login))
        .route("/api/logout", post(logout))
}
