//! Run with `cargo run --example editor`.
use tree_sitter_incremental_highlight::{Document, Patch};

fn apply(colors: &mut [u8], patch: &Patch) {
    colors[patch.start..patch.start + patch.length].fill(0);
    for span in &patch.spans {
        colors[span.start..span.start + span.length].fill(span.style);
    }
}

fn main() -> Result<(), String> {
    let source = "<script>const title = 'Żółć 🙂';</script>";
    let mut document = Document::new("html")?;
    let first = document.update(source, 0, false)?;
    let mut colors = vec![0; source.encode_utf16().count()];
    apply(&mut colors, &first);

    let needle = "Żółć 🙂";
    let byte_start = source.find(needle).unwrap();
    let start = source[..byte_start].encode_utf16().count();
    let end = start + needle.encode_utf16().count();
    let replacement = "Łódź";
    // Keep the color buffer aligned with the editor's text BEFORE applying a patch.
    colors.splice(start..end, vec![0; replacement.encode_utf16().count()]);
    let edited = source.replacen(needle, replacement, 1);
    let patch = document.update(&edited, first.revision, false)?;
    apply(&mut colors, &patch);

    let fresh = Document::new("html")?.update(&edited, 0, true)?;
    let mut expected = vec![0; edited.encode_utf16().count()];
    apply(&mut expected, &fresh);
    assert_eq!(colors, expected);
    assert!(!patch.full);
    println!(
        "revision {}: repaint {} UTF-16 units, {} spans",
        patch.revision,
        patch.length,
        patch.spans.len()
    );

    // If a worker result was dropped, use the last APPLIED revision to recover.
    let missed = document.update(source, patch.revision, false)?;
    let recovered = document.update(&edited, patch.revision, false)?;
    assert!(recovered.full);
    assert!(recovered.revision > missed.revision);
    Ok(())
}
