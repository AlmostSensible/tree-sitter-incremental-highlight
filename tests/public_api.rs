//! Consumer-side contract checks: no access to parser internals.
use tree_sitter_incremental_highlight::{Document, Patch, languages};

fn colors(text: &str, patch: &Patch) -> Vec<u8> {
    let mut result = vec![0; text.encode_utf16().count()];
    assert!(patch.start + patch.length <= result.len());
    for span in &patch.spans {
        assert!(span.style <= 7);
        assert!(span.start >= patch.start);
        assert!(span.start + span.length <= patch.start + patch.length);
        result[span.start..span.start + span.length].fill(span.style);
    }
    result
}

#[test]
fn public_revision_recovery_and_theme_contract() -> Result<(), String> {
    let source = "<script>const title = 'Żółć 🙂';</script>";
    let mut doc = Document::new("html")?;
    let first = doc.update(source, 0, false)?;
    assert!(first.full);
    assert_eq!(first.length, source.encode_utf16().count());
    let unchanged = doc.update(source, first.revision, false)?;
    assert_eq!(unchanged.length, 0);
    assert!(unchanged.spans.is_empty());
    let theme = doc.update(source, unchanged.revision, true)?;
    assert!(theme.full);
    assert_eq!(theme.queried_bytes, 0);
    assert_eq!(colors(source, &first), colors(source, &theme));

    let changed = source.replace("Żółć 🙂", "a\0🙂");
    let _dropped = doc.update(&changed, theme.revision, false)?;
    let resync = doc.update(&changed, theme.revision, false)?;
    assert!(resync.full);
    assert_eq!(resync.queried_bytes, 0);
    let fresh = Document::new("html")?.update(&changed, 0, true)?;
    assert_eq!(colors(&changed, &resync), colors(&changed, &fresh));
    let empty = doc.update("", resync.revision, false)?;
    assert_eq!(empty.length, 0);
    assert!(empty.spans.is_empty());
    Ok(())
}

#[test]
fn exported_registry_constructs_every_language_and_rejects_unknown() {
    assert_eq!(languages::LANGUAGES.len(), 30);
    for language in languages::LANGUAGES {
        assert!(Document::new(language).is_ok(), "{language}");
    }
    assert!(Document::new("unknown-language").is_err());
}
