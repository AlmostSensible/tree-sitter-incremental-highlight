# tree-sitter-incremental-highlight

Incremental UTF-16 syntax highlighting built on Tree-sitter.

**0.1.0-beta.1** — an independent AlmostSensible project, not an official
Tree-sitter package. Extracted from the PoliSSH editor. Tree-sitter provides the
parser, grammar trees and queries; this library provides persistent document
state, incremental query regions, cached nested injections and editor patches.

```rust
use tree_sitter_incremental_highlight::Document;

fn main() -> Result<(), String> {
    let mut document = Document::new("json")?;
    let first = document.update("{\"enabled\":true}", 0, false)?;
    let changed = document.update("{\"enabled\":false}", first.revision, false)?;
    assert!(!changed.full);
    Ok(())
}
```

## Use

Rust 1.88 or later and a C/C++ compiler are required to build the pinned grammars.
Use an exact beta tag/revision while the API is being established:

```toml
[dependencies]
tree-sitter-incremental-highlight = { git = "https://github.com/almostsensible/tree-sitter-incremental-highlight", tag = "v0.1.0-beta.1" }
```

The crate has no PoliSSH, Swift, AppKit, UniFFI or JavaScript runtime dependency.
All 28 grammar packages are included in this beta; optional per-language build
features are not implemented yet. This release is distributed through GitHub;
crates.io publication is a separate step.

## Editor contract

Keep one `Document` per language/editor and serialize updates. Apply the text
edit first, preserving and shifting existing color attributes. Then reset the
foreground color in `patch.start..patch.start + patch.length` to your default
and paint the returned spans. All positions are **absolute UTF-16 code units**,
not bytes or Unicode scalar indices. Text input remains UTF-8.

Pass the revision of the last **applied** patch. If a result was dropped, a stale
revision requests a complete repaint. Do not apply a worker result to text that
has changed since its request. `force_full` repaints unchanged text from cached
colors without querying/parsing, useful when changing themes. An unchanged
normal update returns an empty patch. A full patch includes the entire text.

Styles: `0` default, `1` keyword, `2` string, `3` key/number/constant,
`4` function, `5` type/builtin, `6` comment, `7` added diff text. Choose your own
palette. The eight styles and overlap priorities are part of this beta's contract;
this is not a drop-in replacement for every upstream capture scope.

Runnable examples:

```sh
cargo run --locked --release --example editor
# macOS: builds an example C bridge and verifies actual NSTextStorage colors
bash scripts/test-swift.sh
```

The [Swift example](examples/swift/README.md) is an adapter demonstration using
JSON over a C ABI. Its serialization overhead is excluded from Rust benchmarks;
the bridge is not a promised stable ABI or a production binding package.

## Languages and injections

Bash, C, C++, C#, CSS, Diff, Dockerfile, Go, HTML, INI/.env, Java, JavaScript,
JSON, JSX, Kotlin, Makefile, Markdown, Nginx, PHP, Python, Ruby, Rust, SCSS, SQL,
Swift, TOML, TSX, TypeScript, XML/SVG and YAML: 30 variants, 28 grammar packages.
The `Document` constructor accepts the lowercase keys in `languages::LANGUAGES`.

Cached child documents handle HTML scripts/styles (including typed JSON and
TypeScript scripts), PHP → HTML → JS/CSS, named PHP heredoc/nowdoc, Markdown
fences and HTML blocks, tagged JS/TS/JSX/TSX templates with interpolation holes,
and Dockerfile/Makefile shell bodies. Every supported language can appear in a
Markdown fence. Aliases such as `js`, `py`, `sh`, `yml` and `{.python}` resolve
through the registry. Unknown embedded languages keep host colors; recursion
stops at eight child layers. Disjoint injection regions preserve source byte
positions and line endings by masking excluded content before parsing.

## Limits

Incremental parsing/querying/painting is not an O(edit-size) guarantee. Full
UTF-8 text submission, prefix/suffix diff, UTF-16 mapping, injection discovery
and cache maintenance can still inspect or copy document-sized data. Changes
across broad scopes, large replacements and malformed syntax can cause complete
refreshes. Queries rerun over whole neighboring syntax islands, not just changed
bytes. There is no semantic/LSP coloring, local-variable resolution, regex,
JSDoc/phpdoc grammar or arbitrary template framework support.

Local edits can be faster than upstream `tree-sitter-highlight` because we keep
state and return a small patch. Initial highlighting is not universally faster;
this does not make the underlying Tree-sitter parser faster. See the
[benchmark methodology and raw baseline](benchmarks/README.md).

## Development

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --release --workspace
cargo package --locked
python3 scripts/benchmark.py
```

CI runs Linux/macOS, the minimum Rust version, package verification, the Swift
example and the full comparison suite. Tests compare every UTF-16 color against
fresh parsing after edits, including Unicode, CRLF, invalid syntax, dropped
revisions, themes and nested languages. Hosted CI records timings as artifacts;
it enforces correctness, fixture hashes and patch/query locality. Enforce a
50% timing regression limit on dedicated matching hardware with `--timing` and
its own `--baseline` JSON.

See [CONTRIBUTING.md](CONTRIBUTING.md) for compatibility requirements and
[CHANGELOG.md](CHANGELOG.md) for releases.

## License and provenance

Our implementation is MIT licensed. Tree-sitter, grammars and borrowed grammar
queries retain their original licenses, including Apache-2.0 for the INI grammar.
Full notices are in [THIRD-PARTY-NOTICES.txt](THIRD-PARTY-NOTICES.txt); these
notices also ship inside the Cargo package. [Provenance](docs/PROVENANCE.md)
describes the extraction and query sources. The experimental Rope prototype and
the application code are not included in this release.
