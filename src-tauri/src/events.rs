

use serde::Serialize;

pub const PING_PONG: &str = "zw://ping/pong";

#[derive(Debug, Clone, Serialize)]
pub struct PongPayload {
    pub echo: String,
}
