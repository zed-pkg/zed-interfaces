// Independently authored TypeSpec authority for revision-pinned CLI install receipts.
// Generated JSON Schema is comparison evidence only; authored.schema.json is a peer authority.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace ZedGitCliInstall;

@id("GitCliInstallReceiptV1")
model GitCliInstallReceiptV1 {
  schema_version: uint32;
  source: string;
  revision: string;
  binary: string;
  installed_path: string;
  sha256: string;
  manifest: string;
  flags_contract: string | null;
}
