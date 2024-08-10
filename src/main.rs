use rgate::{run_proxy, run_dashboard};
use clap::Parser;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use tokio::sync::broadcast;
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

    #[arg(short, long, default_value_t = 9000, help = "The port on which the proxy server will listen")]
    port: u16,

    #[arg(short = 'd', long = "dashboard-port", default_value_t = 9001, help = "The port on which the dashboard will listen")]
    dashboard_port: u16,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let base_url = Url::parse(&args.url).expect("Invalid URL");

    let state = Arc::new(Mutex::new(VecDeque::new()));
    let proxy_state = state.clone();
    let (ws_sender, _) = broadcast::channel(100);
    let ws_sender_clone = ws_sender.clone();

    // Display the startup message
    println!(
        "Proxying {} on http://localhost:{}",
        args.url, args.port
    );

    let proxy_task = tokio::spawn(async move {
        run_proxy(proxy_state, base_url, ws_sender, args.port).await;
    });

    let dashboard_state = state.clone();

    let dashboard_task = tokio::spawn(async move {
        run_dashboard(dashboard_state, ws_sender_clone, args.url, args.port, args.dashboard_port).await;
    });

    // Handle the Result from the joined tasks
    if let (Err(e1), Err(e2)) = tokio::join!(proxy_task, dashboard_task) {
        eprintln!("Proxy task failed: {:?}", e1);
        eprintln!("Dashboard task failed: {:?}", e2);
    }
}
