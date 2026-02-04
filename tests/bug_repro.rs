#[cfg(test)]
mod bug_repro {
    use bytes::Bytes;
    use flate2::{write::GzEncoder, Compression};
    use rgate::{proxy_handler, LogEntry};
    use reqwest::Client;
    use std::collections::VecDeque;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tokio::sync::broadcast;
    use url::Url;
    use warp::Filter;
    use warp::http::{Response, StatusCode};

    // A mock server that sends a corrupted gzipped response
    async fn corrupted_gzip_server(original_data_bytes: Bytes) -> Result<impl warp::Reply, warp::Rejection> {
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(&original_data_bytes).unwrap();
        let mut gzipped_bytes = e.finish().unwrap();

        // Corrupt the gzipped bytes by flipping a bit
        let len = gzipped_bytes.len();
        if let Some(byte) = gzipped_bytes.get_mut(len / 2) {
            *byte = !*byte;
        }

        let response: Response<warp::hyper::body::Bytes> = Response::builder()
            .status(StatusCode::OK)
            .header("content-encoding", "gzip")
            .body(gzipped_bytes.into())
            .unwrap();
        Ok(response)
    }

    #[tokio::test]
    async fn test_corrupted_gzip_response() {
        let state = Arc::new(Mutex::new(VecDeque::<LogEntry>::new()));
        let (ws_sender, _) = broadcast::channel(100);
        let client = Client::new();

        let original_data = "This is some data that will be gzipped and then corrupted.";
        let original_data_bytes_for_server = Bytes::from(original_data); // Clone for server
        let original_data_bytes_for_assertion = Bytes::from(original_data); // Clone for assertion

        // Start a mock server that sends a corrupted gzipped response
        let server = warp::serve(
            warp::any()
                .map(move || original_data_bytes_for_server.clone())
                .and_then(corrupted_gzip_server),
        );
        let (addr, server_task) = server.bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::spawn(server_task);

        let base_url = Url::parse(&format!("http://{}", addr)).unwrap();

        let req = warp::http::Request::builder()
            .method("GET")
            .uri("/")
            .body(Bytes::new())
            .unwrap();

        let resp = proxy_handler(client, state.clone(), base_url.clone(), req, ws_sender.clone())
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let log = state.lock().unwrap();
        let log_entry = log.front().unwrap();

        // Expect the response body in the log to be a lossy string representation of the *corrupted gzipped bytes*,
        // not the original uncompressed data, because the proxy_handler currently tries to lossily convert gzipped data
        // when decompression fails.
        let mut e = GzEncoder::new(Vec::new(), Compression::default());
        e.write_all(&original_data_bytes_for_assertion).unwrap();
        let mut expected_gzipped_bytes = e.finish().unwrap();

        // Corrupt the expected gzipped bytes in the same way as the server
        let len = expected_gzipped_bytes.len();
        if let Some(byte) = expected_gzipped_bytes.get_mut(len / 2) {
            *byte = !*byte;
        }

        assert_eq!(
            log_entry.response_body,
            format!("Gzip decompression failed for URI: {}", base_url)
        );
    }
}
