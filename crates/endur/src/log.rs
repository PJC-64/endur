use std::fmt::Debug;

use serde::{Deserialize, Serialize};

use crate::snapshots::CaptureStatus;

#[derive(Debug, Serialize, Deserialize)]
pub enum Operation {
    Snapshot {
        repo: String,
        op: Option<CaptureStatus>,
        error: Option<String>,
        latency: f32,
    },
}

impl Operation {
    pub fn should_log(&self) -> bool {
        match self {
            Operation::Snapshot {
                repo: _,
                op,
                error,
                latency: _,
            } => op.is_some() || error.is_some(),
        }
    }

    pub fn log_str(&mut self) -> String {
        serde_json::to_string(self).expect("Couldn't serialize to JSON")
    }
}
