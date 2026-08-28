/** Generated from a route-map JSON. Do not edit by hand. */

export type HttpMethod = "GET" | "POST" | "PUT" | "PATCH" | "DELETE" | "HEAD" | "OPTIONS";
export type RpcTransport = "http" | "tcp" | "websocket";

export const SERVICE = "zed-api-server" as const;

export const Routes = {
  "healthz": {
    key: "healthz",
    path: "/healthz" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: undefined as ((p: Record<string, never>) => string) | undefined,
  },
  "get_package": {
    key: "get_package",
    path: "/v1/packages/{org}/{name}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string; "name": string }) => "/v1/packages/{org}/{name}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "get_version": {
    key: "get_version",
    path: "/v1/packages/{org}/{name}/versions/{version}" as const,
    methods: ["GET", "PUT"] as const,
    transports: ["http", "tcp"] as const,
    buildPath: (p: { "org": string; "name": string; "version": string }) => "/v1/packages/{org}/{name}/versions/{version}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "get_artifact": {
    key: "get_artifact",
    path: "/v1/artifacts/{sha256}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "sha256": string }) => "/v1/artifacts/{sha256}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "search": {
    key: "search",
    path: "/v1/search" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: undefined as ((p: Record<string, never>) => string) | undefined,
  },
  "list_packages": {
    key: "list_packages",
    path: "/v1/packages" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: undefined as ((p: Record<string, never>) => string) | undefined,
  },
  "semantic_search": {
    key: "semantic_search",
    path: "/v1/search/semantic" as const,
    methods: ["POST"] as const,
    transports: ["http"] as const,
    buildPath: undefined as ((p: Record<string, never>) => string) | undefined,
  },
  "upsert_embedding": {
    key: "upsert_embedding",
    path: "/v1/packages/{org}/{name}/embedding" as const,
    methods: ["PUT"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string; "name": string }) => "/v1/packages/{org}/{name}/embedding".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "get_file": {
    key: "get_file",
    path: "/v1/files/{org}/{name}/{version}/{path}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string; "name": string; "version": string; "path": string }) => "/v1/files/{org}/{name}/{version}/{path}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "yank": {
    key: "yank",
    path: "/v1/packages/{org}/{name}/versions/{version}/yank" as const,
    methods: ["POST"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string; "name": string; "version": string }) => "/v1/packages/{org}/{name}/versions/{version}/yank".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "claim_org": {
    key: "claim_org",
    path: "/v1/orgs" as const,
    methods: ["POST"] as const,
    transports: ["http"] as const,
    buildPath: undefined as ((p: Record<string, never>) => string) | undefined,
  },
  "get_audit": {
    key: "get_audit",
    path: "/v1/orgs/{org}/audit" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string }) => "/v1/orgs/{org}/audit".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "verify_audit": {
    key: "verify_audit",
    path: "/v1/orgs/{org}/audit/verify" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string }) => "/v1/orgs/{org}/audit/verify".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "registry_events": {
    key: "registry_events",
    path: "/v1/ws" as const,
    methods: ["GET"] as const,
    transports: ["websocket"] as const,
    buildPath: undefined as ((p: Record<string, never>) => string) | undefined,
  },
  "cdn_github_object": {
    key: "cdn_github_object",
    path: "/github/{owner}/{repo}/{tag}/{filename}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "owner": string; "repo": string; "tag": string; "filename": string }) => "/github/{owner}/{repo}/{tag}/{filename}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "cdn_package_object": {
    key: "cdn_package_object",
    path: "/packages/{org}/{name}/{version}/{filename}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "org": string; "name": string; "version": string; "filename": string }) => "/packages/{org}/{name}/{version}/{filename}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "cdn_content_object": {
    key: "cdn_content_object",
    path: "/artifacts/{sha256}.{ext}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "sha256": string; "ext": string }) => "/artifacts/{sha256}.{ext}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
  "github_release_asset": {
    key: "github_release_asset",
    path: "/{owner}/{repo}/releases/download/{tag}/{asset}" as const,
    methods: ["GET"] as const,
    transports: ["http"] as const,
    buildPath: (p: { "owner": string; "repo": string; "tag": string; "asset": string }) => "/{owner}/{repo}/releases/download/{tag}/{asset}".replace(/\{([^}]+)\}/g, (_, n) => encodeURIComponent(String((p as Record<string, string>)[n]))),
  },
} as const;

export type RouteName = keyof typeof Routes;

export interface RouteTypes {
  "healthz": { path: Record<string, never>; query: Record<string, never>; body: void; response: unknown };
  "get_package": { path: { "org": string; "name": string }; query: Record<string, never>; body: void; response: unknown };
  "get_version": { path: { "org": string; "name": string; "version": string }; query: Record<string, never>; body: void; response: unknown };
  "get_artifact": { path: { "sha256": string }; query: Record<string, never>; body: void; response: unknown };
  "search": { path: Record<string, never>; query: { "q"?: string; "tag"?: Array<string>; "limit"?: number }; body: void; response: unknown };
  "list_packages": { path: Record<string, never>; query: Record<string, never>; body: void; response: unknown };
  "semantic_search": { path: Record<string, never>; query: Record<string, never>; body: void; response: unknown };
  "upsert_embedding": { path: { "org": string; "name": string }; query: Record<string, never>; body: void; response: unknown };
  "get_file": { path: { "org": string; "name": string; "version": string; "path": string }; query: Record<string, never>; body: void; response: unknown };
  "yank": { path: { "org": string; "name": string; "version": string }; query: Record<string, never>; body: void; response: unknown };
  "claim_org": { path: Record<string, never>; query: Record<string, never>; body: void; response: unknown };
  "get_audit": { path: { "org": string }; query: Record<string, never>; body: void; response: unknown };
  "verify_audit": { path: { "org": string }; query: Record<string, never>; body: void; response: unknown };
  "registry_events": { path: Record<string, never>; query: Record<string, never>; body: void; response: unknown };
  "cdn_github_object": { path: { "owner": string; "repo": string; "tag": string; "filename": string }; query: Record<string, never>; body: void; response: unknown };
  "cdn_package_object": { path: { "org": string; "name": string; "version": string; "filename": string }; query: Record<string, never>; body: void; response: unknown };
  "cdn_content_object": { path: { "sha256": string; "ext": string }; query: Record<string, never>; body: void; response: unknown };
  "github_release_asset": { path: { "owner": string; "repo": string; "tag": string; "asset": string }; query: Record<string, never>; body: void; response: unknown };
}

/** Adding a map key without a handler is a TypeScript error. */
export type RouteHandlers<Ctx> = {
  [K in RouteName]: (ctx: Ctx, args: {
    path: RouteTypes[K]["path"];
    query: RouteTypes[K]["query"];
    body: RouteTypes[K]["body"];
  }) => Promise<RouteTypes[K]["response"]> | RouteTypes[K]["response"];
};

export function lookup<K extends RouteName>(key: K): (typeof Routes)[K] {
  return Routes[key];
}

