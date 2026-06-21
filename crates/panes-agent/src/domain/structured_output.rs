#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuredOutputMode {
    JsonSchema,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredOutputContract {
    pub name: String,
    pub schema: serde_json::Value,
    pub mode: StructuredOutputMode,
}

impl StructuredOutputContract {
    pub fn json_schema(name: impl Into<String>, schema: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            schema,
            mode: StructuredOutputMode::JsonSchema,
        }
    }
}
