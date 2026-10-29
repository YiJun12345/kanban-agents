use tokio::sync::broadcast;
use uuid::Uuid;

use crate::domain::{LogLine, Task};

/// Something happened on the board that browsers should hear about
#[derive(Clone, Debug)]
pub enum BoardEvent {
    TaskUpdated(Task),
    Log { task_id: Uuid, line: LogLine },
}

/// Sending half of the broadcast channel; every subscriber gets every event
pub type Events = broadcast::Sender<BoardEvent>;
