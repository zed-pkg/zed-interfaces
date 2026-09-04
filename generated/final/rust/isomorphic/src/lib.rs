//! Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.

#[derive(Clone, Debug, PartialEq)]
pub struct PageQuery {
    pub cursor: Option<String>,
    pub limit: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProblemDetails {
    pub detail: Option<String>,
    pub request_id: String,
    pub status: i64,
    pub title: String,
    pub type: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequestMeta {
    pub locale: Option<String>,
    pub request_id: String,
    pub trace_id: String,
}
