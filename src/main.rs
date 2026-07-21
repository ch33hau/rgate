use clap::Parser;
use rgate::{run_dashboard, run_proxy, LogEntry};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc};
use url::Url;

#[derive(Parser, Debug)]
#[command(
    name = "rgate",
    version,
    author,
    about = "A simple HTTP proxy and logging tool"
)]
struct Args {
    #[arg(help = "The base URL to which requests will be proxied")]
    url: String,

    #[arg(
        short,
        long,
        default_value_t = 9000,
        help = "The port on which the proxy server will listen"
    )]
    port: u16,

    #[arg(
        short = 'd',
        long = "dashboard-port",
        default_value_t = 9001,
        help = "The port on which the dashboard will listen"
    )]
    dashboard_port: u16,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let base_url = Url::parse(&args.url).expect("Invalid URL");

    let (log_sender, mut log_receiver) = mpsc::channel::<LogEntry>(100);
    let (ws_sender, _) = broadcast::channel(100);
    let ws_sender_clone_for_proxy = ws_sender.clone();
    let ws_sender_clone_for_logger = ws_sender.clone();

    // Display the startup message
    println!("Proxying {} on http://localhost:{}", args.url, args.port);

    let proxy_task = tokio::spawn(async move {
        run_proxy(log_sender, base_url, ws_sender_clone_for_proxy, args.port).await;
    });

    let dashboard_log_state = Arc::new(Mutex::new(VecDeque::new()));
    let dashboard_log_state_clone_for_dashboard = dashboard_log_state.clone();
    let dashboard_log_state_clone_for_logger = dashboard_log_state.clone();

    // Central logging task: receives logs from proxy, stores them, and forwards to dashboard WebSocket
    let logger_task = tokio::spawn(async move {
        while let Some(log_entry) = log_receiver.recv().await {
            {
                let mut state_guard = dashboard_log_state_clone_for_logger.lock().unwrap();
                if state_guard.len() >= 100 {
                    state_guard.pop_front();
                }
                state_guard.push_back(log_entry.clone());
            }
            // Forward to dashboard websocket
            let _ = ws_sender_clone_for_logger.send(log_entry);
        }
    });

    let dashboard_task = tokio::spawn(async move {
        run_dashboard(
            dashboard_log_state_clone_for_dashboard,
            ws_sender,
            args.url,
            args.port,
            args.dashboard_port,
        )
        .await;
    });

    // Handle the Result from the joined tasks
    if let (Err(e1), Err(e2), Err(e3)) = tokio::join!(proxy_task, dashboard_task, logger_task) {
        eprintln!("Proxy task failed: {:?}", e1);
        eprintln!("Dashboard task failed: {:?}", e2);
        eprintln!("Logger task failed: {:?}", e3);
    }
}

#[cfg(test)]
mod args_tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_valid_args() {
        let args = Args::try_parse_from(&["rgate", "http://localhost:8080"]).unwrap();
        assert_eq!(args.url, "http://localhost:8080");
        assert_eq!(args.port, 9000); // Default value
        assert_eq!(args.dashboard_port, 9001); // Default value
    }

    #[test]
    fn test_custom_ports() {
        let args = Args::try_parse_from(&[
            "rgate",
            "http://localhost:8080",
            "--port",
            "8000",
            "--dashboard-port",
            "8001",
        ])
        .unwrap();
        assert_eq!(args.url, "http://localhost:8080");
        assert_eq!(args.port, 8000);
        assert_eq!(args.dashboard_port, 8001);
    }

    #[test]
    fn test_missing_url() {
        let err = Args::try_parse_from(&["rgate"]).unwrap_err();
        assert!(err
            .to_string()
            .contains("required arguments were not provided"));
    }

    #[test]
    fn test_invalid_port_value() {
        let err =
            Args::try_parse_from(&["rgate", "http://localhost:8080", "--port", "abc"]).unwrap_err();
        assert!(err.to_string().contains("invalid digit"));
    }

    #[test]
    fn test_invalid_dashboard_port_value() {
        let err =
            Args::try_parse_from(&["rgate", "http://localhost:8080", "--dashboard-port", "xyz"])
                .unwrap_err();
        assert!(err.to_string().contains("invalid digit"));
    }
}
