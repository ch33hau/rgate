#[cfg(test)]
mod integration_tests {
    use bytes::Bytes;
    use reqwest::Client;
    use rgate::{handle_websocket, proxy_handler, LogEntry};
    use serde_json::Value;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};
    use tokio::sync::{broadcast, mpsc};
    use url::Url;
    use warp::http::StatusCode;
    use warp::test::{request, ws};
    use warp::Filter;
    #[tokio::test]
    async fn test_proxy_handler_get_request() {
        let (log_sender, mut log_receiver) = mpsc::channel::<LogEntry>(1);
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();
        let base_url = Url::parse("https://httpbin.org").unwrap();

        let req = warp::http::Request::builder()
            .method("GET")
            .uri("/get")
            .body(Bytes::new())
            .unwrap();

        let resp = proxy_handler(client, log_sender, base_url, req, ws_sender.clone())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body();
        let _body_str = String::from_utf8_lossy(&body); // Convert to a String

        // Receive the log entry from the channel
        let log_entry: LogEntry = log_receiver.recv().await.unwrap();

        assert!(log_entry
            .response_body
            .contains("\"url\": \"https://httpbin.org/get\""));
    }

    #[tokio::test]
    async fn test_proxy_handler_post_request() {
        let (log_sender, mut log_receiver) = mpsc::channel::<LogEntry>(1);
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();
        let base_url = Url::parse("https://httpbin.org").unwrap();

        let req = warp::http::Request::builder()
            .method("POST")
            .uri("/post")
            .body(Bytes::from(r#"{"name":"test"}"#))
            .unwrap();

        let resp = proxy_handler(client, log_sender, base_url, req, ws_sender.clone())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        let json_body: Value = serde_json::from_str(&body_str).unwrap();
        let expected_json = serde_json::json!({
            "name": "test"});
        assert_eq!(json_body["json"], expected_json);

        // Receive the log entry from the channel
        let log_entry: LogEntry = log_receiver.recv().await.unwrap();
        let response_json: Value = serde_json::from_str(&log_entry.response_body).unwrap();
        let expected_json = serde_json::json!({
            "name": "test"});
        assert_eq!(response_json["json"], expected_json);
    }

    #[tokio::test]
    async fn test_dashboard_logs() {
        let dashboard_log_state = Arc::new(Mutex::new(VecDeque::<LogEntry>::new()));
        let (ws_sender, _) = broadcast::channel::<LogEntry>(100);

        // Manually push a log entry to the state
        let test_log_entry = LogEntry {
            method: "GET".to_string(),
            uri: "/test".to_string(),
            headers: vec![],
            body: "".to_string(),
            response_status: 200,
            response_headers: vec![],
            response_body: "Test Body".to_string(),
            response_time: 10,
        };
        dashboard_log_state
            .lock()
            .unwrap()
            .push_back(test_log_entry.clone());

        // Create the logs route filter, capturing the dashboard_log_state
        let logs_route = warp::path("logs").map(move || {
            let state_guard = dashboard_log_state.lock().unwrap();
            let logs: Vec<LogEntry> = state_guard.iter().cloned().collect();
            warp::reply::json(&rgate::Log { requests: logs })
        });

        let resp = request()
            .method("GET")
            .path("/logs")
            .reply(&logs_route)
            .await;

        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body();
        let body_str = String::from_utf8_lossy(&body);

        assert!(body_str.contains("\"method\":\"GET\""));
        assert!(body_str.contains("\"uri\":\"/test\""));
        assert!(body_str.contains("\"response_body\":\"Test Body\""));
    }

    #[tokio::test]
    async fn test_websocket_connection() {
        let (ws_sender_to_client, _) = broadcast::channel::<LogEntry>(100);

        let ws_sender_to_client_clone = ws_sender_to_client.clone();
        let ws_route = warp::path("ws")
            .and(warp::ws())
            .map(move |ws: warp::ws::Ws| {
                let ws_sender = ws_sender_to_client_clone.clone(); // Clone the sender inside the closure
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

        // Send a log entry, mimicking the central logger task
        ws_sender_to_client.send(log_entry.clone()).unwrap();

        let msg = ws_client.recv().await.expect("Failed to receive message");
        let received_log: LogEntry = serde_json::from_str(msg.to_str().unwrap()).unwrap();

        assert_eq!(received_log.uri, log_entry.uri);
        assert_eq!(received_log.response_status, log_entry.response_status);

        ws_client.send(warp::ws::Message::close()).await;
    }

    #[tokio::test]
    async fn test_proxy_header_manipulation() {
        let (log_sender, mut log_receiver) = mpsc::channel::<LogEntry>(1);
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();
        let base_url = Url::parse("https://httpbin.org").unwrap();

        let req = warp::http::Request::builder()
            .method("GET")
            .uri("/get")
            .header("Host", "some-host.com")
            .header("Content-Length", "123")
            .header("X-Custom-Header", "custom-value")
            .body(Bytes::new())
            .unwrap();

        let resp = proxy_handler(client, log_sender, base_url, req, ws_sender.clone())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        // Receive the log entry from the channel
        let log_entry: LogEntry = log_receiver.recv().await.unwrap();

        let response_json: Value = serde_json::from_str(&log_entry.response_body).unwrap();
        let received_headers = &response_json["headers"];

        // Assert that Content-Length is NOT present in headers received by httpbin.org
        assert!(!received_headers.get("Content-Length").is_some());

        // Assert that X-Custom-Header IS present
        assert_eq!(received_headers["X-Custom-Header"], "custom-value");
    }

    #[tokio::test]
    async fn test_proxy_query_forwarding() {
        let (log_sender, mut log_receiver) = mpsc::channel::<LogEntry>(1);
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();
        let base_url = Url::parse("https://httpbin.org").unwrap();

        let req = warp::http::Request::builder()
            .method("GET")
            .uri("/get?param1=value1&param2=value2")
            .body(Bytes::new())
            .unwrap();

        let resp = proxy_handler(client, log_sender, base_url, req, ws_sender.clone())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        // Receive the log entry from the channel
        let log_entry: LogEntry = log_receiver.recv().await.unwrap();

        assert!(log_entry.uri.contains("param1=value1"));
        assert!(log_entry.uri.contains("param2=value2"));
        assert!(log_entry.uri.contains("/get"));
    }
}
