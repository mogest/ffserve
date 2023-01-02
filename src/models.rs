use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use uuid::Uuid;

#[derive(Clone)]
pub struct Job {
    pub id: Uuid,
    pub source_url: String,
    pub dest_url: String,
    pub state: State,
    pub error_message: Option<String>,
    pub completed_at: Option<Instant>,
}

pub type MutexedJobs = Arc<Mutex<HashMap<Uuid, Job>>>;

pub enum FileType {
    Input,
    Output,
}

#[derive(Debug, Clone)]
pub enum State {
    Waiting,
    Processing,
    Done,
    Error,
}

pub fn build_path(id: Uuid, file_type: FileType) -> String {
    match file_type {
        FileType::Input => id.to_string(),
        FileType::Output => format!("{id}.webm"),
    }
}
