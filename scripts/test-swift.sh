#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked --release -p highlight-example-bridge
mkdir -p results
swiftc examples/swift/Editor.swift -I examples/swift/include -L target/release -lhighlight_example_bridge -o results/swift-example
DYLD_LIBRARY_PATH="$PWD/target/release${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" results/swift-example
