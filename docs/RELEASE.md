# 0.1 beta release procedure

Release identity: `0.1.0-beta.1`, Git tag `v0.1.0-beta.1`.

Before tagging, verify CI for the target commit (Linux/macOS stable and Rust
1.88), the all-language/embedding comparison job, Cargo package verification and
the macOS Swift/AppKit example. Review README limits, changelog and notices.
`cargo package --locked` produces the source archive without publishing it.

For performance-sensitive changes, additionally run isolated same-hardware
benchmarks with `python3 scripts/benchmark.py --baseline PATH --timing`; retain
raw results and review any slower scenarios. GitHub runner timings are not a
substitute for this controlled check.

Create an immutable annotated tag after checks pass and mark the GitHub release
as a **prerelease**. Attach the verified `.crate` source archive and its SHA-256
checksum. Do not move a published tag; corrections need another beta version.
Do not enable automatic crates.io publishing in this beta. GitHub Git dependency
installation uses the release tag or exact commit; Cargo.lock resolves it.

The original editor should pin the exact release commit under its existing
dependency alias, regenerate its lockfile, rebuild UniFFI and run native editor
and integration tests. A library release does not automatically release the app.

For future releases, update package version, changelog, example install tag and
this procedure together. Compatibility expectations are in CONTRIBUTING.md.
