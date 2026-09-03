// Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.

export const contractVersion="ores.validation.v2" as const;
export const contractScope="server" as const;

export interface RegistryLeaseRow {
  readonly acquiredAt: string;
  readonly expiresAt: string;
  readonly fencingToken: number;
  readonly holderId: string;
  readonly resourceKey: string;
}

export interface RegistryPackageRow {
  readonly createdAt: string;
  readonly description?: string;
  readonly id: string;
  readonly name: string;
  readonly namespace: string;
  readonly updatedAt: string;
}

export interface RegistryVersionRow {
  readonly artifactSha256: string;
  readonly artifactSizeBytes: number;
  readonly id: string;
  readonly packageId: string;
  readonly publishedAt: string;
  readonly version: string;
  readonly yanked: boolean;
}
