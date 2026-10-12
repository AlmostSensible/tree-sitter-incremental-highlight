#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --locked --release -p highlight-example-bridge
mkdir -p results
swiftc examples/swift/Editor.swift -I examples/swift/include -L target/release target/release/libhighlight_example_bridge.a -Xlinker -lc++ -Xlinker -liconv -o results/swift-example
results/swift-example
