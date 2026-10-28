#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostRegisterSessionRequest {
    pub agent_id: String,
    pub ref_kind: String,
    #[serde(rename = "ref")]
    pub session_ref: String,
    pub harness: String,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_root: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostRegisterSessionResponse {
    pub ok: bool,
    pub registered: bool,
    pub agent_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LivenessSession {
    #[serde(default = "default_harness")]
    pub harness: String,
    #[serde(rename = "sessionId")]
    pub session_id: String,
    #[serde(rename = "canonicalRoot")]
    pub canonical_root: String,
    pub state: String,
    #[serde(default, rename = "lastActivityMs", skip_serializing_if = "Option::is_none")]
    pub last_activity_ms: Option<i64>,
}

fn default_harness() -> String {
    "opencode".to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostFeedLivenessRequest {
    pub seq: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sessions: Option<Vec<LivenessSession>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostFeedLivenessResponse {
    pub accepted: bool,
    #[serde(rename = "snapshotRequested")]
    pub snapshot_requested: bool,
    pub tracked: usize,
}
