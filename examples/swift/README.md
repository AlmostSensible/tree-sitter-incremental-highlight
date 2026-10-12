# Swift / AppKit adapter example

Run on macOS with Xcode command-line tools and Rust installed:

```sh
bash scripts/test-swift.sh
```

The Rust workspace builds an example-only `cdylib` with an owned opaque document
handle. `Editor.swift` uses its C header/module map, decodes JSON patches and
paints real `NSTextStorage` attributes. It verifies nested HTML/JavaScript,
Unicode deletion including surrogate pairs, equivalence to a fresh complete
parse, theme refresh without queries, empty input and embedded NUL bytes.

Own and free each document/result once; serialize calls for each handle. Input
is passed with its explicit UTF-8 length; output ranges are UTF-16. The example
has no UI window and does not implement asynchronous scheduling. In an editor,
run parsing on a serial worker, discard results for obsolete text and only
advance `appliedRevision` after applying the patch on the main thread.

The JSON C bridge demonstrates integration with minimal binding machinery. It
is not a stable ABI, a benchmark of FFI latency, or a production Swift package.
Production clients can provide their own typed C/UniFFI adapter around the public
Rust API (PoliSSH uses UniFFI).
