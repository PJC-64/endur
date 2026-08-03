use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct SnapshotInfo {
    pub commit_hash: String,
    pub base_hash: String,
    pub timestamp: i64,
    pub message: String,
    pub files_changed: usize,
}
