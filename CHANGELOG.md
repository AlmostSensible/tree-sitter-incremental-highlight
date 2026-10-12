# Changelog

## 0.1.0-beta.1

First standalone beta, extracted from a native editor's Rust highlighting backend.

- Persistent Tree-sitter documents with incremental queries and UTF-16 patches.
- 30 language variants backed by 28 pinned grammar packages.
- Cached nested language injections and revision recovery for dropped updates.
- Eight style categories and cached full repaint for theme changes.
- Fresh-parse correctness oracles, runnable Rust/Swift examples and 101
  comparison workloads against tree-sitter-highlight 0.25.10.
- MIT implementation license with preserved upstream grammar/query notices.

Initial beta API; use exact tags/revisions. All grammars are included. No stable
C ABI, semantic coloring, optional grammar features or edit-sized memory bound.
