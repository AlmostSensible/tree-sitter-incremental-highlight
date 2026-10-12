import AppKit
import CIncrementalHighlight

struct Span: Decodable { let start: Int; let length: Int; let style: UInt8 }
struct Patch: Decodable {
    let start: Int; let length: Int; let spans: [Span]
    let revision: UInt64; let full: Bool; let queriedBytes: Int
}

final class Highlighter {
    private let handle: UnsafeMutableRawPointer
    private(set) var appliedRevision: UInt64 = 0
    init(language: String) {
        guard let handle = language.withCString({ ih_document_new($0) }) else {
            fatalError("Unsupported language: \(language)")
        }
        self.handle = handle
    }
    deinit { ih_document_free(handle) }
    // Call on one serial queue. A real editor should parse away from the main
    // thread and apply only results whose text revision still matches its storage.
    func update(_ text: String, forceFull: Bool = false) throws -> Patch {
        let bytes = Array(text.utf8) + [0]
        let result = bytes.withUnsafeBufferPointer {
            ih_document_update(handle, $0.baseAddress!, bytes.count - 1, appliedRevision, forceFull)
        }!
        defer { ih_string_free(result) }
        return try JSONDecoder().decode(Patch.self, from: Data(String(cString: result).utf8))
    }
    func apply(_ patch: Patch, to storage: NSTextStorage) {
        let palette: [NSColor] = [.textColor, .systemRed, .systemBlue, .systemTeal,
                                  .systemPurple, .systemOrange, .systemGray, .systemGreen]
        precondition(patch.start >= 0 && patch.start + patch.length <= storage.length)
        storage.beginEditing()
        storage.addAttribute(.foregroundColor, value: palette[0],
                             range: NSRange(location: patch.start, length: patch.length))
        for span in patch.spans {
            precondition(span.start >= patch.start && span.start + span.length <= patch.start + patch.length)
            storage.addAttribute(.foregroundColor, value: palette[Int(span.style)],
                                 range: NSRange(location: span.start, length: span.length))
        }
        storage.endEditing()
        appliedRevision = patch.revision // Only advance after painting succeeded.
    }
}

let source = "<script>const title = 'Żółć 🙂';</script>"
let storage = NSTextStorage(string: source)
let engine = Highlighter(language: "html")
engine.apply(try engine.update(source), to: storage)
let range = (storage.string as NSString).range(of: "Żółć 🙂")
storage.replaceCharacters(in: range, with: "Łódź")
let patch = try engine.update(storage.string)
precondition(!patch.full)
engine.apply(patch, to: storage)
let fresh = NSTextStorage(string: storage.string)
let oracle = Highlighter(language: "html")
oracle.apply(try oracle.update(fresh.string), to: fresh)
for index in 0..<storage.length {
    let actual = storage.attribute(.foregroundColor, at: index, effectiveRange: nil) as! NSColor
    let expected = fresh.attribute(.foregroundColor, at: index, effectiveRange: nil) as! NSColor
    precondition(actual == expected, "Color mismatch at UTF-16 position \(index)")
}
let theme = try engine.update(storage.string, forceFull: true)
precondition(theme.full && theme.queriedBytes == 0)
engine.apply(theme, to: storage)
// Exercise an empty document and an embedded NUL without C-string truncation.
storage.replaceCharacters(in: NSRange(location: 0, length: storage.length), with: "")
engine.apply(try engine.update(""), to: storage)
let nulSource = "<script>const title = 'a\0b🙂';</script>"
storage.replaceCharacters(in: NSRange(location: 0, length: 0), with: nulSource)
engine.apply(try engine.update(nulSource), to: storage)
print("Swift/AppKit: Unicode deletion, fresh-color equivalence, theme refresh, empty/NUL input passed")
