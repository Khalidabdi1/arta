//! Output formatting

use crate::engine::executor::ExecutionResult;
use crate::output::human::format_human;
use crate::output::json::format_json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

pub fn format_output(result: &ExecutionResult, format: &OutputFormat) -> String {
    match format {
        OutputFormat::Human => format_human(result),
        OutputFormat::Json => format_json(result),
    }
}
