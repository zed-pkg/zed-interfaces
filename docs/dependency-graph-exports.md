# Dependency graph export representations

`zpkg/dependency-graph/v1` has one semantic graph document and one semantic
`graph_digest`. The registry may project that finalized document into multiple
byte representations without resolving dependencies again.

The original package-version endpoint remains canonical for JSON, YAML, TOML,
DOT, and Mermaid:

```text
GET /v1/packages/{org}/{name}/versions/{version}/dependency-graph?view=declared&format=json
```

Additional representations use the additive export route:

```text
GET /v1/packages/{org}/{name}/versions/{version}/dependency-graph/export/{format}
```

| Representation | Route values | Media type | Lossless |
| --- | --- | --- | --- |
| JSON5 | `json5` | `application/vnd.zpkg.dependency-graph.v1+json5` | yes |
| XML | `xml` | `application/vnd.zpkg.dependency-graph.v1+xml` | yes |
| CSV | `csv` | `text/csv; charset=utf-8` | no; node/edge analytics projection |
| MessagePack | `msgpack`, `messagepack`, `mpk` | `application/vnd.zpkg.dependency-graph.v1+msgpack` | yes |
| Protocol Buffers | `protobuf`, `proto`, `pb` | `application/vnd.zpkg.dependency-graph.v1+protobuf` | yes |

Every successful response carries:

- `X-Zpkg-Graph-Digest`: the semantic digest shared across representations.
- `ETag`: a strong validator for the exact response bytes.
- `X-Zpkg-Graph-Authoritative`: `true` for reversible interchange formats and
  `false` for CSV.
- `Content-Disposition`: an immutable-version download filename.
- `Cache-Control: public, max-age=31536000, immutable`.

## Representation rules

### JSON5

The body begins with `//` comments describing the schema and digest, followed by
the canonical JSON document unchanged. JSON is a valid JSON5 value, so JSON5
parsers retain the comments while canonical JSON verification can strip them.

### XML

The XML projection maps declared and resolved graph fields to named elements and
attributes. Collections retain canonical document order, and XML-reserved or
attribute-whitespace characters are escaped deterministically.

### CSV

CSV is an RFC 4180 node/edge table intended for spreadsheets and analytics. It
repeats graph identity on each row and stores feature lists as JSON strings. It
does not attempt to flatten all provenance and projection metadata, so it must
never be used as the source of a lock or semantic graph digest.

### MessagePack

MessagePack encodes the canonical JSON value with named map keys. Decoding it
produces the same document fields and semantic digest as JSON.

### Protocol Buffers

The stable typed schema is committed at
`proto/zpkg_dependency_graph_v1.proto`. Field numbers are append-only. Declared
and resolved graphs occupy separate `oneof` arms so clients cannot conflate an
unresolved requirement with a selected package version.
