use crate::{Document, Run, languages, merge_runs};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};
use tree_sitter::{Query, QueryCursor, StreamingIterator, Tree};

pub(crate) struct Layer {
    pub key: (usize, String, usize),
    pub ranges: Vec<Range<usize>>,
    pub document: Box<Document>,
}
struct Spec {
    key: (usize, String, usize),
    ranges: Vec<Range<usize>>,
}

fn specs(language: &str, tree: &Tree, source: &str) -> Result<Vec<Spec>, String> {
    // A conservative lexical precheck avoids walking large plain-language trees.
    // It only rules out impossible injections; the syntax query decides the rest.
    let possible = match language {
        "html" => {
            source
                .as_bytes()
                .windows(7)
                .any(|w| w.eq_ignore_ascii_case(b"<script"))
                || source
                    .as_bytes()
                    .windows(6)
                    .any(|w| w.eq_ignore_ascii_case(b"<style"))
        }
        "dockerfile" => {
            source
                .as_bytes()
                .windows(3)
                .any(|w| w.eq_ignore_ascii_case(b"run") || w.eq_ignore_ascii_case(b"cmd"))
                || source
                    .as_bytes()
                    .windows(10)
                    .any(|w| w.eq_ignore_ascii_case(b"entrypoint"))
        }
        "php" => {
            source.contains("?>")
                || source.contains("<<<")
                || !source.trim_start().starts_with("<?")
        }
        "markdown" => source.contains("```") || source.contains("~~~") || source.contains('<'),
        "javascript" | "jsx" | "typescript" | "tsx" => source.contains('`'),
        _ => true,
    };
    if !possible {
        return Ok(Vec::new());
    }
    let text = languages::injections(language);
    if text.is_empty() {
        return Ok(Vec::new());
    }
    static QUERIES: OnceLock<Mutex<HashMap<String, Arc<Query>>>> = OnceLock::new();
    let query = {
        let mut cache = QUERIES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|e| e.to_string())?;
        if let Some(query) = cache.get(language) {
            query.clone()
        } else {
            let query = Arc::new(
                Query::new(&languages::grammar(language)?.language, text)
                    .map_err(|e| format!("{language} injections: {e}"))?,
            );
            cache.insert(language.to_owned(), query.clone());
            query
        }
    };
    let mut result: Vec<Spec> = Vec::new();
    let mut counts = HashMap::new();
    let mut claimed = HashSet::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(found) = matches.next() {
        let settings = query.property_settings(found.pattern_index);
        let fixed = settings
            .iter()
            .find(|p| p.key.as_ref() == "injection.language")
            .and_then(|p| p.value.as_deref());
        let dynamic = found
            .captures
            .iter()
            .find(|c| query.capture_names()[c.index as usize] == "injection.language")
            .map(|c| &source[c.node.byte_range()]);
        let include_children = settings
            .iter()
            .any(|p| p.key.as_ref() == "injection.include-children");
        let combined = settings
            .iter()
            .any(|p| p.key.as_ref() == "injection.combined");
        let selected = languages::resolve_injection(dynamic.or(fixed).unwrap_or(""));
        let mut ranges = Vec::new();
        for capture in found
            .captures
            .iter()
            .filter(|c| query.capture_names()[c.index as usize] == "injection.content")
        {
            let range = capture.node.byte_range();
            // Earlier, specific HTML MIME rules take precedence over the default.
            if !claimed.insert((range.start, range.end)) {
                continue;
            }
            if selected.is_none() {
                continue;
            }
            if include_children {
                ranges.push(range);
            } else {
                let mut start = range.start;
                let mut walk = capture.node.walk();
                for child in capture.node.named_children(&mut walk) {
                    if start < child.start_byte() {
                        ranges.push(start..child.start_byte());
                    }
                    start = child.end_byte();
                }
                if start < range.end {
                    ranges.push(start..range.end);
                }
            }
        }
        let Some(selected) = selected else {
            continue;
        };
        if ranges.is_empty() {
            continue;
        }
        if combined
            && let Some(spec) = result
                .iter_mut()
                .find(|spec| spec.key.0 == found.pattern_index && spec.key.1 == selected)
        {
            spec.ranges.extend(ranges);
            continue;
        }
        let ordinal = counts.entry((found.pattern_index, selected)).or_insert(0);
        result.push(Spec {
            key: (found.pattern_index, selected.into(), *ordinal),
            ranges,
        });
        *ordinal += 1;
    }
    for spec in &mut result {
        spec.ranges.sort_by_key(|r| r.start);
    }
    Ok(result)
}

// Replace excluded bytes with ASCII spaces, preserving offsets and line breaks.
// Child colors are mapped in bytes back to the real source before UTF-16 conversion.
fn masked(source: &str, ranges: &[Range<usize>]) -> String {
    let start = ranges[0].start;
    let end = ranges.last().unwrap().end;
    let mut bytes = source.as_bytes()[start..end].to_vec();
    let mut previous = start;
    for range in ranges {
        for byte in &mut bytes[previous - start..range.start - start] {
            if *byte != b'\n' && *byte != b'\r' {
                *byte = b' ';
            }
        }
        previous = range.end;
    }
    String::from_utf8(bytes).expect("Injection ranges are scalar boundaries")
}

pub(crate) fn update(
    document: &mut Document,
    source: &str,
    dirty: &mut Range<usize>,
    edit: &tree_sitter::InputEdit,
) -> Result<usize, String> {
    if document.depth >= 8 {
        document.rendered = document.colors.clone();
        return Ok(0);
    }
    let specs = specs(&document.language, document.tree.as_ref().unwrap(), source)?;
    let mut old: HashMap<_, _> = std::mem::take(&mut document.layers)
        .into_iter()
        .map(|layer| (layer.key.clone(), layer))
        .collect();
    let mut bytes = 0;
    let mut colors = Vec::new();
    let mut cleared = Vec::new();
    for spec in specs {
        let offset = spec.ranges[0].start;
        let text = masked(source, &spec.ranges);
        // Do not recurse into an unchanged self-injection.
        if spec.key.1 == document.language && text == source {
            continue;
        }
        let mut child = if let Some(layer) = old.remove(&spec.key) {
            layer.document
        } else {
            Box::new(Document::new_at_depth(&spec.key.1, document.depth + 1)?)
        };
        let changed = child.source != text || child.tree.is_none();
        let patch = child.update(&text, child.revision, false)?;
        bytes += patch.queried_bytes;
        if changed {
            dirty.start = dirty.start.min(offset + child.last_range.start);
            dirty.end = dirty.end.max(offset + child.last_range.end);
        }
        let mut run_index = 0;
        for range in &spec.ranges {
            cleared.push(range.clone());
            while run_index < child.rendered.len()
                && offset + child.rendered[run_index].end <= range.start
            {
                run_index += 1;
            }
            for run in child.rendered[run_index..]
                .iter()
                .take_while(|run| offset + run.start < range.end)
            {
                let start = (offset + run.start).max(range.start);
                let end = (offset + run.end).min(range.end);
                if start < end {
                    colors.push(Run {
                        start,
                        end,
                        style: run.style,
                    });
                }
            }
        }
        document.layers.push(Layer {
            key: spec.key,
            ranges: spec.ranges,
            document: child,
        });
    }
    // Removed/retagged injections must clear their old colors, including a deleted fence.
    let delta = edit.new_end_byte as isize - edit.old_end_byte as isize;
    let map = |byte: usize| {
        if byte <= edit.start_byte {
            byte
        } else if byte >= edit.old_end_byte {
            byte.saturating_add_signed(delta)
        } else {
            edit.new_end_byte
        }
    };
    for layer in old.into_values() {
        for range in layer.ranges {
            dirty.start = dirty.start.min(map(range.start));
            dirty.end = dirty.end.max(map(range.end).min(source.len()));
        }
    }
    colors.extend(clear(&document.colors, cleared));
    colors.sort_by_key(|r| r.start);
    document.rendered = merge_runs(colors);
    Ok(bytes)
}
fn clear(colors: &[Run], mut ranges: Vec<Range<usize>>) -> Vec<Run> {
    ranges.sort_by_key(|range| range.start);
    let mut result = Vec::new();
    let mut index = 0;
    for run in colors {
        let mut start = run.start;
        while index < ranges.len() && ranges[index].end <= start {
            index += 1;
        }
        for range in ranges[index..]
            .iter()
            .take_while(|range| range.start < run.end)
        {
            if start < range.start {
                result.push(Run {
                    start,
                    end: range.start.min(run.end),
                    style: run.style,
                });
            }
            start = start.max(range.end);
        }
        if start < run.end {
            result.push(Run {
                start,
                end: run.end,
                style: run.style,
            });
        }
    }
    result
}
