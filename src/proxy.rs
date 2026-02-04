use bytes::Bytes;
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::broadcast;
use url::Url;
use warp::http::{HeaderMap, Response, StatusCode};
use warp::path::FullPath;
use warp::Filter;

pub type SharedState = Arc<Mutex<VecDeque<LogEntry>>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Log {
    pub requests: Vec<LogEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LogEntry {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: String,
    pub response_time: u128,
}

pub async fn proxy_handler(
    client: Client,
    state: SharedState,
    base_url: Url,
    req: warp::http::Request<Bytes>,
    ws_sender: broadcast::Sender<LogEntry>,
) -> Result<Response<Bytes>, warp::Rejection> {
    let start_time = Instant::now();

    let mut new_uri = base_url.clone();
    new_uri.set_path(req.uri().path());
    new_uri.set_query(req.uri().query());

    let method = req.method().clone();
    let headers = req.headers().clone();
    let body_bytes = req.body().clone();
    let request_body = String::from_utf8_lossy(&body_bytes).to_string();

    let mut new_req_builder = client.request(method.clone(), new_uri.to_string());

    for (key, value) in headers.iter() {
        if key.as_str() != "host" && key.as_str() != "content-length" {
            new_req_builder = new_req_builder.header(key, value);
        }
    }

    let new_req = match new_req_builder.body(body_bytes.to_vec()).build() {
        Ok(req) => req,
        Err(e) => {
            eprintln!("Error building request: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Bytes::from("Error building request"))
                .unwrap());
        }
    };

    let response = match client.execute(new_req).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error executing request: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::BAD_GATEWAY)
                .body(Bytes::from("Bad Gateway"))
                .unwrap());
        }
    };
    let status = response.status();
    let response_headers = response.headers().clone();
    let response_body_bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Error reading response body: {}", e);
            return Ok(Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Bytes::from("Error reading response body"))
                .unwrap());
        }
    };

    let response_body = if response_headers
        .get("content-encoding")
        .map(|v| v == "gzip")
        .unwrap_or(false)
    {
        let mut decoder = GzDecoder::new(&response_body_bytes[..]);
        let mut decompressed_body = String::new();
        if decoder.read_to_string(&mut decompressed_body).is_ok() {
            decompressed_body
        } else {
            String::from_utf8_lossy(&response_body_bytes).to_string()
        }
    } else {
        String::from_utf8_lossy(&response_body_bytes).to_string()
    };


    let request_headers = headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();
    let response_headers_vec = response_headers
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect();

    let response_time = start_time.elapsed().as_millis();

    let log_entry = LogEntry {
        method: method.to_string(),
        uri: new_uri.to_string(),
        headers: request_headers,
        body: request_body,
        response_status: status.as_u16(),
        response_headers: response_headers_vec,
        response_body: response_body.clone(),
        response_time,
    };

    {
        let mut state_guard = state.lock().unwrap();
        if state_guard.len() >= 100 {
            state_guard.pop_front();
        }
        state_guard.push_back(log_entry.clone());
    }

    if ws_sender.receiver_count() > 0 {
        let _ = ws_sender.send(log_entry.clone());
    }

    // Print a detailed log entry
    println!(
        "{} {} {}ms {}",
        log_entry.method, log_entry.uri, log_entry.response_time, log_entry.response_status
    );

    let mut response_builder = Response::builder().status(status);
    for (key, value) in response_headers.iter() {
        response_builder = response_builder.header(key, value);
    }

    Ok(response_builder.body(response_body_bytes).unwrap())
}

pub async fn run_proxy(
    state: SharedState,
    base_url: Url,
    ws_sender: broadcast::Sender<LogEntry>,
    port: u16,
) {
    let client = Client::new();
    let state_filter = warp::any().map(move || state.clone());
    let client_filter = warp::any().map(move || client.clone());
    let base_url_filter = warp::any().map(move || base_url.clone());
    let ws_sender_filter = warp::any().map(move || ws_sender.clone());

    let proxy_route = warp::path::full()
        .and(warp::method())
        .and(warp::header::headers_cloned())
        .and(warp::body::bytes())
        .and(state_filter)
        .and(client_filter)
        .and(base_url_filter)
        .and(ws_sender_filter)
        .and_then(
            |path: FullPath,
             method,
             headers: HeaderMap,
             body,
             state,
             client,
             base_url,
             ws_sender| async move {
                let mut req_builder = warp::http::Request::builder()
                    .method(method)
                    .uri(path.as_str());

                for (key, value) in headers.iter() {
                    if key.as_str() != "host" && key.as_str() != "content-length" {
                        req_builder = req_builder.header(key, value);
                    }
                }

                let req = match req_builder.body(body) {
                    Ok(req) => req,
                    Err(e) => {
                        eprintln!("Error building request: {}", e);
                        let res: Result<Response<Bytes>, warp::Rejection> = Ok(Response::builder()
                            .status(StatusCode::INTERNAL_SERVER_ERROR)
                            .body(Bytes::from("Error building request"))
                            .unwrap());
                        return res;
                    }
                };
                proxy_handler(client, state, base_url, req, ws_sender).await
            },
        );

    warp::serve(proxy_route).run(([0, 0, 0, 0], port)).await;
}
