use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub operation: String,
    pub problem: String,
    pub safe_alternative: String,
    pub severity: Severity,
    pub line_number: Option<usize>,
}

impl Violation {
    pub fn new(
        operation: impl Into<String>,
        problem: impl Into<String>,
        safe_alternative: impl Into<String>,
    ) -> Self {
        Self {
            operation: operation.into(),
            problem: problem.into(),
            safe_alternative: safe_alternative.into(),
            severity: Severity::Error,
            line_number: None,
        }
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line_number = Some(line);
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }
}
