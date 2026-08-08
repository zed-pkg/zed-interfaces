# zed_interfaces (Dart)

Front-end contract types for the [zed-pkg](https://github.com/zed-pkg) registry:
immutable classes with `fromJson`/`toJson` for the registry HTTP API and the
sync stream.

```dart
import 'package:zed_interfaces/zed_interfaces.dart';

final metadata = PackageMetadata.fromJson(
  jsonDecode(response.body) as Map<String, dynamic>,
);
for (final version in metadata.versions) {
  print('${metadata.org}/${metadata.name}@$version');
}
```

Every file in `lib/` is **generated** from the JSON Schemas in
[`schemas/`](../../schemas), which are themselves generated from the Rust crate
in [`../rust`](../rust). Do not edit them; change the Rust type and run:

```sh
cargo run --locked --example generate_schemas   # from the repository root
npm run codegen
```

This slice deliberately covers less than the Rust crate — only what a browser or
Flutter client decodes. The demarcation lives in
[`schemas/index.json`](../../schemas/index.json).

Unknown enum values throw `FormatException` rather than decoding to a fallback:
an unrecognized variant means the client is older than the server, and silently
mapping it to something else would hide that.
