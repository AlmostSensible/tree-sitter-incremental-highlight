# Benchmarks

Run `python3 scripts/benchmark.py` from the repository root. It emits JSON with
all raw samples, fixture SHA-256 values, patch/query sizes and resolved child
languages, plus the original generated fixtures in `results/`.

The default covers all 30 variants, 1,000/10,000-entry fixtures, JSON/PHP malformed
cases, a large PHP function and 19 embedded workloads: **101 scenarios**.
`HIGHLIGHT_LANGUAGES=json,php,html,markdown` focuses a run; `HIGHLIGHT_SIZES=1000`
reduces local/embedded sizes. Malformed/function cases always use 10,000 entries.
`HIGHLIGHT_AUDIT_ONLY=1` prints resolved upstream injection sequences without timings.

We compare against tree-sitter-highlight **0.25.10** using the same parser,
grammars, queries, injection resolver and eight output styles. Every upstream
iterator is fully consumed. Raw event consumption and normalized UTF-16 spans
are reported separately. Incremental patches are checked after all 17 alternating
edits against the engine's independent fresh complete result. The upstream engine
has its own fresh-result oracle; every embedding must resolve the expected child
language sequence before timing.

The engines use different overlap priorities (for example JSON key captures),
so `cross_engine_style_differences` are recorded; we do not claim identical
capture semantics. There are 15 recorded samples after 2 warmups with rotated
execution order. Grammar/query setup is measured separately from initial load
and sequential edit latency. Source mutations, hashes, fixtures and correctness
checks are outside timing. Rust Release mode; no FFI, JSON bridge, UI, painting or
debounce is timed. Tests with simplified generated fixtures do not predict every
real-world project or total editor frame latency.

The checked-in [historical raw baseline](baselines/macos-arm64-2026-10-12.json)
shows faster local embedded edits for recognized languages (roughly 1.4–6x in
10,000-entry scenarios), mixed initial-load results and a slight slowdown for
unknown fenced languages. It is evidence for these workloads, not a universal
speed promise or a claim to accelerate the underlying Tree-sitter parser.

Correctness, fixture hashes and query/patch locality gates run on every machine.
A local update must query fewer than 500 bytes, repaint fewer than 500 UTF-16
units, emit fewer than 50 spans and avoid full patches. Malformed cases must
conservatively refresh all. The Rust tests additionally cover broad boundary
changes that legitimately need full refreshes.

Hosted CI captures timings as artifacts without gating wall-clock medians.
For controlled regression tracking, save a report from dedicated hardware and
run `python3 scripts/benchmark.py --baseline PATH.json --timing`. This enforces
a maximum 50% median load/edit increase and refuses mismatched hardware or missing
baseline scenarios. Match compiler/build settings too; avoid simultaneous builds
and record a reviewed explanation when replacing a baseline. Keep failing
reports rather than silently relaxing thresholds.
