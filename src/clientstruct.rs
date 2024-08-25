use serde::{Deserialize, Serialize};

#[derive(Debug,Clone, Serialize, Deserialize)]
pub struct ClientBase {
    pub timestamp: u64,
    pub choco_installed: String,
    pub cookie_count: i64,
    pub login_count: i64,
}
