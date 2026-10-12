# Beta extraction validation

Local validation on 2026-10-12, macOS arm64:

- Rust stable (1.95.0) and minimum Rust 1.88.0 workspace tests passed:
  15 engine tests, 2 consumer/public-API tests, 1 example bridge test and
  1 README doctest. Fixtures cover all 30 language variants and nested injections.
- Format and strict Clippy checks passed on stable; strict Clippy on 1.88 exposed
  a duplicate test-only `cfg(test)` attribute inherited from the extraction.
  The duplicate was removed and the 1.88 lint check passed without suppressions.
- Rust example passed Unicode deletion, shifted attributes and missed-revision recovery.
- Swift/AppKit static bridge example passed real NSTextStorage color equivalence,
  nested highlighting, Unicode deletion, theme repaint, empty input and embedded NUL.
- `cargo package --locked` compiled the packaged source with its own tests/queries
  and complete upstream notices. Only library sources, fixtures, Rust example and
  licensing/API documentation are packaged; SSH/application code is excluded.
- Full 101-workload Rust Release comparison passed fixture SHA checks, independent
  fresh-color oracles, expected upstream child-language sequences, local query/patch
  limits, malformed recovery and the 50% median load/edit regression gate against
  the checked-in matching-hardware baseline. Raw local output remains in `results/`.

The macOS dynamic example bridge initially hit a linker error in the Rust dylib's
LINKEDIT pool. The example now links a static Rust library, verified with swiftc;
no dynamic loader paths or dylib packaging are required.

GitHub CI validates Linux/macOS stable and Linux Rust 1.88, packaged-source builds,
Swift/AppKit integration and all 101 comparison workloads. See the Actions run
for the exact release commit. Hosted timing medians are informational; controlled
same-hardware timing gates are documented in benchmarks/README.md.

These checks establish beta extraction correctness and reproducibility for the
included fixtures. They do not promise semantic coloring, complete upstream scope
parity, universal speedups or stable C/Swift bindings.
