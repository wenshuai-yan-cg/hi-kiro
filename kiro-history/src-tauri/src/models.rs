use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub created_at: i64, // Unix timestamp ms
    pub updated_at: i64, // Unix timestamp ms
    pub messages: Vec<Message>,
    pub model_name: Option<String>,
    pub max_context_pct: Option<f32>,
    pub total_tool_uses: i64,
    pub total_cycles: i64,
    pub total_duration_secs: i64,
    pub source: SessionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Jsonl,
    SqliteV1,
    SqliteV2,
}

impl std::fmt::Display for SessionSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionSource::Jsonl => write!(f, "jsonl"),
            SessionSource::SqliteV1 => write!(f, "sqlite_v1"),
            SessionSource::SqliteV2 => write!(f, "sqlite_v2"),
        }
    }
}
