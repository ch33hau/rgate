mod proxy;
pub use proxy::*;

mod dashboard;
pub use dashboard::*;

mod error;
pub use error::*;

use tokio::sync::mpsc;

pub type LogSender = mpsc::Sender<LogEntry>;
pub type LogReceiver = mpsc::Receiver<LogEntry>;
