/// Generated from a route-map JSON. Do not edit by hand.
library;

const String kService = "zed-api-server";

class RouteMeta {
  const RouteMeta({required this.key, required this.path, required this.methods, this.transports = const ['http']});
  final String key;
  final String path;
  final List<String> methods;
  final List<String> transports;
  String expand(Map<String, String> params) {
    var out = path;
    params.forEach((k, v) {
      out = out.replaceAll('{$k}', Uri.encodeComponent(v));
    });
    return out;
  }
}

abstract final class Routes {
  static const healthz = RouteMeta(key: "healthz", path: "/healthz", methods: ["GET"], transports: ["http"]);
  static const get_package = RouteMeta(key: "get_package", path: "/v1/packages/{org}/{name}", methods: ["GET"], transports: ["http"]);
  static const get_version = RouteMeta(key: "get_version", path: "/v1/packages/{org}/{name}/versions/{version}", methods: ["GET", "PUT"], transports: ["http", "tcp"]);
  static const get_artifact = RouteMeta(key: "get_artifact", path: "/v1/artifacts/{sha256}", methods: ["GET"], transports: ["http"]);
  static const search = RouteMeta(key: "search", path: "/v1/search", methods: ["GET"], transports: ["http"]);
  static const list_packages = RouteMeta(key: "list_packages", path: "/v1/packages", methods: ["GET"], transports: ["http"]);
  static const semantic_search = RouteMeta(key: "semantic_search", path: "/v1/search/semantic", methods: ["POST"], transports: ["http"]);
  static const upsert_embedding = RouteMeta(key: "upsert_embedding", path: "/v1/packages/{org}/{name}/embedding", methods: ["PUT"], transports: ["http"]);
  static const get_file = RouteMeta(key: "get_file", path: "/v1/files/{org}/{name}/{version}/{path}", methods: ["GET"], transports: ["http"]);
  static const yank = RouteMeta(key: "yank", path: "/v1/packages/{org}/{name}/versions/{version}/yank", methods: ["POST"], transports: ["http"]);
  static const claim_org = RouteMeta(key: "claim_org", path: "/v1/orgs", methods: ["POST"], transports: ["http"]);
  static const get_audit = RouteMeta(key: "get_audit", path: "/v1/orgs/{org}/audit", methods: ["GET"], transports: ["http"]);
  static const verify_audit = RouteMeta(key: "verify_audit", path: "/v1/orgs/{org}/audit/verify", methods: ["GET"], transports: ["http"]);
  static const registry_events = RouteMeta(key: "registry_events", path: "/v1/ws", methods: ["GET"], transports: ["websocket"]);
  static const cdn_github_object = RouteMeta(key: "cdn_github_object", path: "/github/{owner}/{repo}/{tag}/{filename}", methods: ["GET"], transports: ["http"]);
  static const cdn_package_object = RouteMeta(key: "cdn_package_object", path: "/packages/{org}/{name}/{version}/{filename}", methods: ["GET"], transports: ["http"]);
  static const cdn_content_object = RouteMeta(key: "cdn_content_object", path: "/artifacts/{sha256}.{ext}", methods: ["GET"], transports: ["http"]);
  static const github_release_asset = RouteMeta(key: "github_release_asset", path: "/{owner}/{repo}/releases/download/{tag}/{asset}", methods: ["GET"], transports: ["http"]);

  static const Map<String, RouteMeta> byKey = {
    "healthz": healthz,
    "get_package": get_package,
    "get_version": get_version,
    "get_artifact": get_artifact,
    "search": search,
    "list_packages": list_packages,
    "semantic_search": semantic_search,
    "upsert_embedding": upsert_embedding,
    "get_file": get_file,
    "yank": yank,
    "claim_org": claim_org,
    "get_audit": get_audit,
    "verify_audit": verify_audit,
    "registry_events": registry_events,
    "cdn_github_object": cdn_github_object,
    "cdn_package_object": cdn_package_object,
    "cdn_content_object": cdn_content_object,
    "github_release_asset": github_release_asset,
  };
}

