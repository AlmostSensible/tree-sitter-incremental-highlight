//! Example-only C bridge: JSON keeps ownership simple, at a serialization cost.
//! This ABI is not part of the library's stable public API.
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use tree_sitter_incremental_highlight::Document;

/// # Safety
/// `language` must point to a valid NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ih_document_new(language: *const c_char) -> *mut Document {
    if language.is_null() {
        return std::ptr::null_mut();
    }
    catch_unwind(|| {
        let language = unsafe { CStr::from_ptr(language) }.to_str().ok()?;
        Document::new(language)
            .ok()
            .map(|doc| Box::into_raw(Box::new(doc)))
    })
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `document` must be a live handle from `ih_document_new`, used serially.
/// `source` must refer to `length` readable UTF-8 bytes (including for length 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ih_document_update(
    document: *mut Document,
    source: *const u8,
    length: usize,
    revision: u64,
    force_full: bool,
) -> *mut c_char {
    let result = catch_unwind(AssertUnwindSafe(|| {
        if document.is_null() || source.is_null() {
            return serde_json::json!({"error": "null input"});
        }
        let bytes = unsafe { std::slice::from_raw_parts(source, length) };
        let source = match std::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => return serde_json::json!({"error": error.to_string()}),
        };
        match unsafe { &mut *document }.update(source, revision, force_full) {
            Ok(patch) => serde_json::json!({
                "start": patch.start, "length": patch.length,
                "revision": patch.revision, "full": patch.full,
                "queriedBytes": patch.queried_bytes,
                "spans": patch.spans.iter().map(|span| serde_json::json!({
                    "start": span.start, "length": span.length, "style": span.style
                })).collect::<Vec<_>>()
            }),
            Err(error) => serde_json::json!({"error": error}),
        }
    }))
    .unwrap_or_else(|_| serde_json::json!({"error": "Rust panic"}));
    CString::new(result.to_string()).unwrap().into_raw()
}

/// # Safety
/// `document` must be null or an unfreed handle from `ih_document_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ih_document_free(document: *mut Document) {
    if !document.is_null() {
        drop(unsafe { Box::from_raw(document) });
    }
}

/// # Safety
/// `text` must be null or an unfreed result from `ih_document_update`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ih_string_free(text: *mut c_char) {
    if !text.is_null() {
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_validates_utf8_and_owns_results() {
        unsafe {
            assert!(ih_document_new(c"unsupported".as_ptr()).is_null());
            let doc = ih_document_new(c"json".as_ptr());
            assert!(!doc.is_null());
            let invalid = [0xff];
            let result = ih_document_update(doc, invalid.as_ptr(), 1, 0, false);
            let json: serde_json::Value =
                serde_json::from_slice(CStr::from_ptr(result).to_bytes()).unwrap();
            assert!(json.get("error").is_some());
            ih_string_free(result);
            let text = b"{\"value\": true}";
            let result = ih_document_update(doc, text.as_ptr(), text.len(), 0, false);
            let json: serde_json::Value =
                serde_json::from_slice(CStr::from_ptr(result).to_bytes()).unwrap();
            assert_eq!(json["full"], true);
            assert!(!json["spans"].as_array().unwrap().is_empty());
            ih_string_free(result);
            ih_document_free(doc);
        }
    }
}
