# @zed-pkg/zed-interfaces

Front-end contract types for the [zed-pkg](https://github.com/zed-pkg) registry:
`interface` declarations for the registry HTTP API and the sync stream, plus a
`*_VALUES` array per enum for validation and pickers.

```ts
import type { PackageMetadata } from "@zed-pkg/zed-interfaces";
import { VCS_VALUES } from "@zed-pkg/zed-interfaces";

const metadata: PackageMetadata = await response.json();
```

Interfaces mirror the wire exactly — snake_case keys, optional where the server
may omit the field — so a parsed response *is* the type, with no adapter layer.

Every `.ts` file here is **generated** from the JSON Schemas in
[`schemas/`](../../schemas), which are themselves generated from the Rust crate
in [`../rust`](../rust). Do not edit them; change the Rust type and run:

```sh
cargo run --locked --example generate_schemas   # from the repository root
npm run codegen
```

This slice deliberately covers less than the Rust crate — only what a browser or
Flutter client decodes. The demarcation lives in
[`schemas/index.json`](../../schemas/index.json).
