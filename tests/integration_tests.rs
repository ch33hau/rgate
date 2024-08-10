#[cfg(test)]
mod integration_tests {
    use rgate::{LogEntry, proxy_handler, handle_websocket};
    use warp::http::{StatusCode, Response};
    use warp::test::{request, ws};
    use std::sync::{Arc, Mutex};
    use std::collections::VecDeque;
    use tokio::sync::broadcast;
    use warp::Filter;
    use reqwest::Client;
    use url::Url;
    use bytes::Bytes;
    use warp::Reply;
    use serde_json::Value;


    #[tokio::test]
    async fn test_proxy_handler_get_request() {
        let state = Arc::new(Mutex::new(VecDeque::<LogEntry>::new()));
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();
        let base_url = Url::parse("https://httpbin.org").unwrap();

        let req = warp::http::Request::builder()
            .method("GET")
            .uri("/get")
            .body(Bytes::new())
            .unwrap();

        let resp = proxy_handler(client, state.clone(), base_url, req, ws_sender.clone()).await.unwrap();
        let resp = Response::from(resp.into_response());

        assert_eq!(resp.status(), StatusCode::OK);
        let resp = Response::from(resp.into_response()); // Convert to Response
        let body = warp::hyper::body::to_bytes(resp.into_body()).await.unwrap(); // Extract the body as Bytes
        let body_str = String::from_utf8_lossy(&body); // Convert to a String

        assert!(body_str.contains("\"url\": \"https://httpbin.org/get\""));

    }

    #[tokio::test]
    async fn test_proxy_handler_post_request() {
        let state = Arc::new(Mutex::new(VecDeque::<LogEntry>::new()));
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();
        let base_url = Url::parse("https://httpbin.org").unwrap();

        let req = warp::http::Request::builder()
            .method("POST")
            .uri("/post")
            .body(Bytes::from(r#"{"name":"test"}"#))
            .unwrap();

        let resp = proxy_handler(client, state.clone(), base_url, req, ws_sender.clone()).await.unwrap();
        let resp = Response::from(resp.into_response());

        assert_eq!(resp.status(), StatusCode::OK);
        let body = warp::hyper::body::to_bytes(resp.into_body()).await.unwrap();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let json_body: Value = serde_json::from_str(&body_str).unwrap();
        let expected_json = serde_json::json!({
            "name": "test"});
        assert_eq!(json_body["json"], expected_json);

    }

    #[tokio::test]
    async fn test_dashboard_logs() {
        let state = Arc::new(Mutex::new(VecDeque::<LogEntry>::new()));

        let logs_route = warp::path("logs")
            .map(move || {
                let state_guard = state.lock().unwrap();
                let logs: Vec<LogEntry> = state_guard.iter().cloned().collect();
                warp::reply::json(&rgate::Log { requests: logs })
            });

        let resp = request()
            .method("GET")
            .path("/logs")
            .reply(&logs_route)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let resp = resp.into_response(); // Convert to warp::http::Response
        let body = warp::hyper::body::to_bytes(resp.into_body()).await.unwrap(); // Convert body to bytes
        let body_str = String::from_utf8_lossy(&body);
        assert!(body_str.contains("\"requests\":[]"));

    }

    #[tokio::test]
    async fn test_websocket_connection() {
        let (ws_sender, _) = broadcast::channel::<LogEntry>(100);

        // Clone ws_sender for usage in the closure and outside it
        let ws_sender_clone_for_closure = ws_sender.clone();
        let ws_sender_clone_for_use_later = ws_sender.clone();


        let ws_route = warp::path("ws")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let ws_sender = ws_sender_clone_for_closure.clone(); // Clone the sender inside the closure
                ws.on_upgrade(move |websocket| handle_websocket(websocket, ws_sender))
            });

        let mut ws_client = ws()
            .path("/ws")
            .handshake(ws_route)
            .await
            .expect("Failed to connect to WebSocket");

        let log_entry = LogEntry {
            method: "GET".to_string(),
            uri: "https://example.com".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: "".to_string(),
            response_status: 200,
            response_headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            response_body: r#"{"message":"hello"}"#.to_string(),
            response_time: 100,
        };

        // Now, you can use the second clone of ws_sender outside the closure
        ws_sender_clone_for_use_later.send(log_entry.clone()).unwrap();

        let msg = ws_client.recv().await.expect("Failed to receive message");
        let received_log: LogEntry = serde_json::from_str(msg.to_str().unwrap()).unwrap();

        assert_eq!(received_log.uri, log_entry.uri);
        assert_eq!(received_log.response_status, log_entry.response_status);

        ws_client.send(warp::ws::Message::close()).await;
    }
}
