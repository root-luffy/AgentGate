use bytes::Bytes;
use serde::Deserialize;
use serde_json::Value;

use crate::error::AppError;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

pub struct InspectedRequest {
    pub rpc: JsonRpcRequest,
    pub tool_name: Option<String>,
}

pub fn inspect(body: &Bytes) -> Result<InspectedRequest, AppError> {
    if body.is_empty() {
        return Ok(InspectedRequest {
            rpc: JsonRpcRequest { jsonrpc: "2.0".into(), id: None, method: String::new(), params: None },
            tool_name: None,
        });
    }
    let rpc: JsonRpcRequest = serde_json::from_slice(body)
        .map_err(|e| AppError::BadRequest(format!("invalid JSON-RPC: {e}")))?;

    let tool_name = if rpc.method == "tools/call" {
        rpc.params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
    } else {
        None
    };

    Ok(InspectedRequest { rpc, tool_name })
}
