use rusty_gate::{run_proxy, run_dashboard};
use clap::Parser;
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;
use tokio::sync::broadcast;
use url::Url;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    url: String,
    #[arg(short, long, default_value_t = 9000)]
    port: u16,
    #[arg(long, default_value_t = 9001)]
    dashboard_port: u16,
}

#[tokio::main]
pub async fn main() {
    let args = Args::parse();
    let base_url = Url::parse(&args.url).expect("Invalid URL");

    let state = Arc::new(Mutex::new(VecDeque::new()));
    let proxy_state = state.clone();
    let (ws_sender, _) = broadcast::channel(100);
    let ws_sender_clone = ws_sender.clone();

    let proxy_task = tokio::spawn(async move {
        run_proxy(proxy_state, base_url, ws_sender, args.port).await;
    });

    let dashboard_state = state.clone();

    let dashboard_task = tokio::spawn(async move {
        run_dashboard(dashboard_state, ws_sender_clone, args.url, args.port, args.dashboard_port).await;
    });

    if let (Err(proxy_err), Err(dashboard_err)) = tokio::join!(proxy_task, dashboard_task) {
        eprintln!("Proxy server error: {:?}", proxy_err);
        eprintln!("Dashboard server error: {:?}", dashboard_err);
    }
}
