#!/bin/sh
set -eu

target="${ZED_PKG_TEST_TARGET:?ZED_PKG_TEST_TARGET is required}"

test -f "$target/schemas/index.json"
test -f "$target/src/rust/Cargo.toml"
test -f "$target/src/dart/lib/zed_interfaces.dart"
test -f "$target/src/ts/index.ts"
