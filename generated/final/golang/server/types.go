// Generated only after independent JSON Schema and TypeSpec agreement. DO NOT EDIT.

package interfaces_server

const ContractVersion="ores.validation.v2"

type RegistryLeaseRow struct {
	AcquiredAt string `json:"acquiredAt"`
	ExpiresAt string `json:"expiresAt"`
	FencingToken int64 `json:"fencingToken"`
	HolderId string `json:"holderId"`
	ResourceKey string `json:"resourceKey"`
}

type RegistryPackageRow struct {
	CreatedAt string `json:"createdAt"`
	Description *string `json:"description,omitempty"`
	Id string `json:"id"`
	Name string `json:"name"`
	Namespace string `json:"namespace"`
	UpdatedAt string `json:"updatedAt"`
}

type RegistryVersionRow struct {
	ArtifactSha256 string `json:"artifactSha256"`
	ArtifactSizeBytes int64 `json:"artifactSizeBytes"`
	Id string `json:"id"`
	PackageId string `json:"packageId"`
	PublishedAt string `json:"publishedAt"`
	Version string `json:"version"`
	Yanked bool `json:"yanked"`
}
