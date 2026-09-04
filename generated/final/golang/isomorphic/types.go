// Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.

package interfaces_isomorphic

const ContractVersion="ores.validation.v2"

type PageQuery struct {
	Cursor *string `json:"cursor,omitempty"`
	Limit int64 `json:"limit"`
}

type ProblemDetails struct {
	Detail *string `json:"detail,omitempty"`
	RequestId string `json:"requestId"`
	Status int64 `json:"status"`
	Title string `json:"title"`
	Type string `json:"type"`
}

type RequestMeta struct {
	Locale *string `json:"locale,omitempty"`
	RequestId string `json:"requestId"`
	TraceId string `json:"traceId"`
}
