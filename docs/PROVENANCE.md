# Provenance

The implementation was extracted on 2026-10-12 from the `polissh-syntax` Rust
crate in PoliSSH. Application code, account/storage data, credentials, native
app benchmarks and the experimental Rope path are excluded. The extracted
engine keeps the tested behavior and its 30-language registry.

Tree-sitter and grammar versions are pinned in Cargo.toml and Cargo.lock.
Most highlighting queries are read directly from those grammar crates. C++ and
SCSS combine their C/CSS bases; JS/TS variants combine appropriate base/JSX
queries. Kotlin and Markdown highlighting queries are supplied by this library;
Dockerfile/Nginx query material follows the corresponding upstream grammars.
The injection queries and editor style mapping are maintained here.

THIRD-PARTY-NOTICES.txt retains upstream authors and license texts for the parser
and all 28 grammar packages. The INI package uses Apache-2.0; most others use MIT.
The SCSS crate release omitted its license file; its upstream MIT text is retained
in the notices. Notices are bundled with the Cargo package and must travel with
redistributed code/binaries as required by the applicable licenses. Transitive
crates keep their own source-package licenses in the Cargo dependency graph.

The historical baseline contains only generated synthetic fixture metadata and
samples from a MacBookPro18,3/arm64. It was collected before extraction from a
dirty PoliSSH checkout; its revision is provenance for those measurements, not a
commit in this repository. Its focused rerun metadata is intentionally preserved.
New reports record this repository's actual commit and dirty state.
