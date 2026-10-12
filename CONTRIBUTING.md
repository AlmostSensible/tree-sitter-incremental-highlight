# Contributing

Treat this repository as a reusable library, even when a change originates in
the original editor. Keep editor/UI policy and application-specific behavior in consumers.

Before opening a pull request, run the commands in README and describe the
observable behavior change. A syntax fix needs a fixture that reproduces it and
an independent fresh-parse oracle after sequential edits. Cover Unicode UTF-16
positions, malformed syntax and affected embedded languages where relevant.

Do not change style IDs, revision recovery, language keys, patch positioning or
injection behavior silently. Describe compatibility changes in CHANGELOG and
update examples/tests. For beta releases, use exact tags/revisions in consumers;
breaking API changes require a new beta version and migration notes. For stable
releases, follow SemVer and use a new minor series before breaking 0.x APIs.

Parser/query/dependency upgrades require the full language and embedded-language
comparison suite and preserving third-party notices. Keep fixture hashes and
benchmarks reviewable; do not regenerate baselines just to remove a failure.
Timings from hosted runners are informational, while correctness/locality gates
must pass. Significant performance changes need repeated isolated measurements
on the same hardware, compiler and workload.

Contributions are provided under the project's MIT license. Retain upstream
licenses for borrowed material. Never add connection credentials, application
storage, generated build products or private production documents as fixtures.
