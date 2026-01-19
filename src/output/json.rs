//! JSON output formatting

use crate::engine::executor::{ExecutionResult, ResultData};
use serde_json::{json, Value};

pub fn format_json(result: &ExecutionResult) -> String {
    let data: Value = match &result.data {
        ResultData::Cpu(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Memory(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Disk(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Network(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::System(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Battery(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Processes(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Files(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Content(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::ActionResult(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::ContextInfo(info) => serde_json::to_value(info).unwrap_or(json!(null)),
        ResultData::Explanation(s) => json!({ "explanation": s }),
        ResultData::Message(s) => json!({ "message": s }),
        ResultData::Multiple(results) => {
            let items: Vec<Value> = results
                .iter()
                .map(|r| serde_json::from_str(&format_json(r)).unwrap_or(json!(null)))
                .collect();
            json!({ "results": items })
        }
        ResultData::Empty => json!({ "empty": true }),
        ResultData::ContainerResult(info) => serde_json::to_value(info).unwrap_or(json!(null)),
    };

    serde_json::to_string_pretty(&data).unwrap_or_else(|_| "{}".to_string())
}
