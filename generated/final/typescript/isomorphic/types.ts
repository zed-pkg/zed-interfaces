// Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.

export const contractVersion="ores.validation.v2" as const;
export const contractScope="isomorphic" as const;

export interface PageQuery {
  readonly cursor?: string;
  readonly limit: number;
}

export interface ProblemDetails {
  readonly detail?: string;
  readonly requestId: string;
  readonly status: number;
  readonly title: string;
  readonly type: string;
}

export interface RequestMeta {
  readonly locale?: string;
  readonly requestId: string;
  readonly traceId: string;
}
