use crate::Log;
use crate::{LogEntry, SharedState};
use futures_util::SinkExt;
use futures_util::StreamExt;
use tokio::sync::broadcast;
use warp::Filter;

pub async fn run_dashboard(
    state: SharedState,
    ws_sender: broadcast::Sender<LogEntry>,
    url: String,
    app_port: u16,       // Application port
    dashboard_port: u16, // Dashboard port
) {
    let logs_state = state.clone();
    let logs = warp::path("logs").map(move || {
        let state_guard = logs_state
            .lock()
            .expect("Mutex poisoned in dashboard logs endpoint");
        let logs: Vec<LogEntry> = state_guard.iter().cloned().collect();
        warp::reply::json(&Log { requests: logs })
    });

    let clear_state = state.clone();
    let clear_logs = warp::path("clear-logs").map(move || {
        let mut state_guard = clear_state
            .lock()
            .expect("Mutex poisoned in dashboard clear logs endpoint");
        state_guard.clear(); // Clear the server-side log
        warp::reply::json(&"Log cleared")
    });

    let dashboard = warp::path("dashboard").map(move || {
        let html = include_str!("../dashboard.html")
            .replace("{{url}}", &url)
            .replace("{{port}}", &app_port.to_string()); // Use application port here
        warp::reply::html(html)
    });

    let ws_route = warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            let ws_sender = ws_sender.clone();
            ws.on_upgrade(move |websocket| handle_websocket(websocket, ws_sender))
        });

    let routes = logs.or(clear_logs).or(dashboard).or(ws_route);

    warp::serve(routes)
        .run(([0, 0, 0, 0], dashboard_port))
        .await;
}

pub async fn handle_websocket(ws: warp::ws::WebSocket, ws_sender: broadcast::Sender<LogEntry>) {
    let mut rx = ws_sender.subscribe();

    let (mut ws_tx, mut ws_rx) = ws.split();

    tokio::spawn(async move { while let Some(Ok(_)) = ws_rx.next().await {} });

    while let Ok(log_entry) = rx.recv().await {
        let msg = serde_json::to_string(&log_entry).unwrap_or_else(|e| {
            eprintln!("Error serializing log entry: {}", e);
            "{}".to_string()
        });
        if let Err(e) = ws_tx.send(warp::ws::Message::text(msg)).await {
            eprintln!("WebSocket send error: {}", e);
            break;
        }
    }
}
