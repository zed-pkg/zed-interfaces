// Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.
import gleam/option.{type Option}

pub const contract_version="ores.validation.v2"

pub type PageQuery {
  PageQuery(
    cursor: Option(String),
    limit: Int,
  )
}

pub type ProblemDetails {
  ProblemDetails(
    detail: Option(String),
    request_id: String,
    status: Int,
    title: String,
    type: String,
  )
}

pub type RequestMeta {
  RequestMeta(
    locale: Option(String),
    request_id: String,
    trace_id: String,
  )
}
