//! Incremental editor coloring. Query whole syntax islands, retain unaffected
//! colors, and return a UTF-16 patch. Malformed syntax conservatively refreshes all.
mod injections;
pub mod languages;

use std::{
    cmp::Reverse,
    collections::{BTreeSet, HashMap},
    ops::Range,
    sync::{Arc, Mutex, OnceLock},
};
use tree_sitter::{InputEdit, Node, Parser, Point, Query, QueryCursor, StreamingIterator, Tree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub length: usize,
    pub style: u8,
}
#[derive(Debug)]
pub struct Patch {
    pub start: usize,
    pub length: usize,
    pub spans: Vec<Span>,
    pub revision: u64,
    pub full: bool,
    pub queried_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Run {
    start: usize,
    end: usize,
    style: u8,
}

pub struct Document {
    parser: Parser,
    query: Arc<Query>,
    language: String,
    tree: Option<Tree>,
    source: String,
    colors: Vec<Run>,
    revision: u64,
    depth: usize,
    layers: Vec<injections::Layer>,
    rendered: Vec<Run>,
    last_range: Range<usize>,
}
impl Document {
    pub fn new(language: &str) -> Result<Self, String> {
        Self::new_at_depth(language, 0)
    }
    fn new_at_depth(language: &str, depth: usize) -> Result<Self, String> {
        static QUERIES: OnceLock<Mutex<HashMap<String, Arc<Query>>>> = OnceLock::new();
        let specification = languages::grammar(language)?;
        let mut parser = Parser::new();
        parser
            .set_language(&specification.language)
            .map_err(|e| e.to_string())?;
        let mut queries = QUERIES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .map_err(|e| e.to_string())?;
        let query = if let Some(query) = queries.get(language) {
            query.clone()
        } else {
            let query = Arc::new(
                Query::new(&specification.language, &specification.highlights)
                    .map_err(|e| format!("{language}: {e}"))?,
            );
            queries.insert(language.to_owned(), query.clone());
            query
        };
        Ok(Self {
            parser,
            query,
            language: language.into(),
            tree: None,
            source: String::new(),
            colors: Vec::new(),
            revision: 0,
            depth,
            layers: Vec::new(),
            rendered: Vec::new(),
            last_range: 0..0,
        })
    }
    pub fn update(
        &mut self,
        source: &str,
        base_revision: u64,
        force_full: bool,
    ) -> Result<Patch, String> {
        let resync = force_full || base_revision != self.revision || self.tree.is_none();
        if self.tree.is_some() && self.source == source {
            return Ok(self.patch(if resync { 0..source.len() } else { 0..0 }, resync, 0));
        }
        let edit = source_edit(&self.source, source);
        let old_region = self.tree.as_ref().map(|tree| {
            island_region(
                tree,
                &self.language,
                edit.start_byte..edit.old_end_byte,
                self.source.len(),
            )
        });
        let malformed = self
            .tree
            .as_ref()
            .is_some_and(|t| t.root_node().has_error());
        if let Some(tree) = &mut self.tree {
            tree.edit(&edit);
        }
        let tree = self
            .parser
            .parse(source, self.tree.as_ref())
            .ok_or("Syntax parsing cancelled")?;
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
        let mut dirty = edit.start_byte..edit.new_end_byte;
        if let Some(old) = old_region {
            dirty.start = dirty.start.min(map(old.start));
            dirty.end = dirty.end.max(map(old.end));
        }
        if let Some(old) = &self.tree {
            for changed in old.changed_ranges(&tree) {
                dirty.start = dirty.start.min(changed.start_byte);
                dirty.end = dirty.end.max(changed.end_byte);
            }
        }
        dirty = if self.tree.is_none() || malformed || tree.root_node().has_error() {
            0..source.len()
        } else {
            island_region(&tree, &self.language, dirty, source.len())
        };
        let fresh = query_colors(&self.query, &tree, source, dirty.clone());
        // Retain/shift cached byte ranges. Captures are normalized into disjoint
        // color runs, so splitting a run at a patch boundary cannot lose scope.
        let mut retained = Vec::with_capacity(self.colors.len() + fresh.len());
        for old in &self.colors {
            // A parent capture may span several syntax islands (e.g. a Markdown
            // heading or adjacent diff lines). Keep both unchanged sides of it.
            let before = Run {
                start: old.start,
                end: old.end.min(edit.start_byte),
                style: old.style,
            };
            let after = Run {
                start: old
                    .start
                    .max(edit.old_end_byte)
                    .saturating_add_signed(delta),
                end: old.end.saturating_add_signed(delta),
                style: old.style,
            };
            for moved in [before, after].into_iter().filter(|r| r.start < r.end) {
                if moved.start < dirty.start {
                    retained.push(Run {
                        start: moved.start,
                        end: moved.end.min(dirty.start),
                        style: moved.style,
                    });
                }
                if moved.end > dirty.end {
                    retained.push(Run {
                        start: moved.start.max(dirty.end),
                        end: moved.end,
                        style: moved.style,
                    });
                }
            }
        }
        retained.extend(fresh);
        retained.sort_by_key(|r| r.start);
        self.colors = merge_runs(retained);
        self.source.clear();
        self.source.push_str(source);
        self.tree = Some(tree);
        self.revision += 1;
        let bytes = dirty.len() + injections::update(self, source, &mut dirty, &edit)?;
        while !source.is_char_boundary(dirty.start) {
            dirty.start -= 1;
        }
        while !source.is_char_boundary(dirty.end) {
            dirty.end += 1;
        }
        let full = resync || dirty == (0..source.len());
        self.last_range = if full { 0..source.len() } else { dirty.clone() };
        Ok(self.patch(if full { 0..source.len() } else { dirty }, full, bytes))
    }
    fn patch(&self, range: Range<usize>, full: bool, queried_bytes: usize) -> Patch {
        let start = self.source[..range.start].encode_utf16().count();
        let region = &self.source[range.clone()];
        let mut offsets = vec![0; region.len() + 1];
        let mut length = 0;
        for (byte, c) in region.char_indices() {
            offsets[byte] = length;
            length += c.len_utf16();
        }
        offsets[region.len()] = length;
        let spans = self
            .rendered
            .iter()
            .filter_map(|run| {
                let begin = run.start.max(range.start);
                let end = run.end.min(range.end);
                (begin < end).then(|| Span {
                    start: start + offsets[begin - range.start],
                    length: offsets[end - range.start] - offsets[begin - range.start],
                    style: run.style,
                })
            })
            .collect();
        Patch {
            start,
            length,
            spans,
            revision: self.revision,
            full,
            queried_bytes,
        }
    }
}

fn merge_runs(runs: Vec<Run>) -> Vec<Run> {
    let mut result: Vec<Run> = Vec::new();
    for run in runs.into_iter().filter(|r| r.start < r.end) {
        if let Some(last) = result
            .last_mut()
            .filter(|r| r.end == run.start && r.style == run.style)
        {
            last.end = run.end;
        } else {
            result.push(run);
        }
    }
    result
}

fn island_region(tree: &Tree, language: &str, range: Range<usize>, length: usize) -> Range<usize> {
    let root = tree.root_node();
    if let Some(node) = root.descendant_for_byte_range(range.start, range.end) {
        let mut node = Some(node);
        while let Some(content) = node {
            if matches!(
                content.kind(),
                "raw_text"
                    | "code_fence_content"
                    | "text"
                    | "heredoc_body"
                    | "nowdoc_body"
                    | "string_fragment"
                    | "shell_text"
            ) && range.start > content.start_byte()
                && range.end < content.end_byte()
                && !languages::injections(language).is_empty()
            {
                return range;
            }
            node = content.parent();
        }
    }

    if root.has_error() {
        return 0..length;
    }
    let container = if language == "json" {
        let mut node = root
            .descendant_for_byte_range(range.start, range.end)
            .unwrap_or(root);
        loop {
            if matches!(node.kind(), "object" | "array") {
                break node;
            }
            match node.parent() {
                Some(parent) => node = parent,
                None => return 0..length,
            }
        }
    } else if language == "php" {
        let mut node = root
            .descendant_for_byte_range(range.start, range.end)
            .unwrap_or(root);
        loop {
            if matches!(
                node.kind(),
                "compound_statement" | "declaration_list" | "program"
            ) {
                break node;
            }
            match node.parent() {
                Some(parent) => node = parent,
                None => break root,
            }
        }
    } else {
        let mut node = root
            .descendant_for_byte_range(range.start, range.end)
            .unwrap_or(root);
        loop {
            if matches!(
                node.kind(),
                "block"
                    | "class_body"
                    | "declaration_list"
                    | "compound_statement"
                    | "statement_block"
                    | "function_body"
                    | "block_mapping"
                    | "block_sequence"
                    | "element"
                    | "Element"
                    | "section"
                    | "changes"
            ) {
                break node;
            }
            match node.parent() {
                Some(parent) => node = parent,
                None => break root,
            }
        }
    };
    let mut cursor = container.walk();
    let nodes: Vec<Node<'_>> = container.named_children(&mut cursor).collect();
    if nodes.is_empty() {
        return 0..length;
    }
    // Neighbors cover punctuation deletion, insertion in gaps and sibling
    // predicates. PHP queries are confined to complete sibling statements/declarations;
    // JSON queries to complete sibling pairs/elements. Error trees refresh fully.
    let first = nodes
        .iter()
        .position(|n| n.end_byte() >= range.start)
        .unwrap_or(nodes.len() - 1)
        .saturating_sub(1);
    let last = (nodes
        .iter()
        .rposition(|n| n.start_byte() <= range.end)
        .unwrap_or(0)
        + 1)
    .min(nodes.len() - 1);
    range.start.min(nodes[first].start_byte())..range.end.max(nodes[last].end_byte()).min(length)
}

fn style(name: &str) -> (u8, u8) {
    if name.starts_with("comment") {
        (6, 100)
    } else if name == "string.special.key" {
        (3, 90)
    } else if name.starts_with("escape") || name == "string.escape" {
        (3, 80)
    } else if name.starts_with("variable.builtin") {
        (5, 70)
    } else if name.starts_with("variable") {
        (0, 60)
    } else if name.starts_with("function") {
        (4, 70)
    } else if name.starts_with("type") || name.starts_with("constructor") {
        (5, 60)
    } else if name.starts_with("keyword") || name.starts_with("tag") {
        (1, 60)
    } else if name.starts_with("constant") || name.starts_with("number") || name == "boolean" {
        (3, 60)
    } else if name.starts_with("property")
        || name.starts_with("attribute")
        || name.starts_with("label")
    {
        (3, 70)
    } else if name.starts_with("diff.plus") || name == "diff.add" {
        (7, 60)
    } else if name.starts_with("diff.minus") || name == "diff.delete" {
        (1, 60)
    } else if name.starts_with("markup.heading") {
        (1, 30)
    } else if name.starts_with("markup.raw") {
        (2, 90)
    } else if name.starts_with("markup.strong") {
        (5, 60)
    } else if name.starts_with("markup.italic") {
        (4, 60)
    } else if name.starts_with("string") {
        (2, 20)
    } else {
        (0, 1)
    }
}
fn query_colors(query: &Query, tree: &Tree, source: &str, range: Range<usize>) -> Vec<Run> {
    if range.is_empty() {
        return Vec::new();
    }
    let mut cursor = QueryCursor::new();
    cursor.set_byte_range(range.clone());
    let mut captures = cursor.captures(query, tree.root_node(), source.as_bytes());
    let mut colors = Vec::new();
    let mut events = Vec::new();
    while let Some((found, index)) = captures.next() {
        let capture = found.captures[*index];
        let bytes = capture.node.byte_range();
        let start = bytes.start.max(range.start);
        let end = bytes.end.min(range.end);
        if start >= end {
            continue;
        }
        let (color, priority) = style(query.capture_names()[capture.index as usize]);
        let id = colors.len();
        colors.push(color);
        let key = (priority, Reverse(bytes.len()), id);
        events.push((start, true, key));
        events.push((end, false, key));
    }
    events.sort_unstable();
    let mut active: BTreeSet<(u8, Reverse<usize>, usize)> = BTreeSet::new();
    let mut previous = range.start;
    let mut runs = Vec::new();
    for (position, add, key) in events {
        if let Some(top) = active.last().filter(|_| previous < position) {
            let color = colors[top.2];
            if color != 0 {
                runs.push(Run {
                    start: previous,
                    end: position,
                    style: color,
                });
            }
        }
        if add {
            active.insert(key);
        } else {
            active.remove(&key);
        }
        previous = position;
    }
    merge_runs(runs)
}
fn point(source: &str, byte: usize) -> Point {
    let prefix = &source.as_bytes()[..byte];
    Point {
        row: prefix.iter().filter(|&&c| c == b'\n').count(),
        column: prefix.len()
            - prefix
                .iter()
                .rposition(|&c| c == b'\n')
                .map_or(0, |n| n + 1),
    }
}
fn source_edit(old: &str, new: &str) -> InputEdit {
    let prefix: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let suffix: usize = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(c, _)| c.len_utf8())
        .sum();
    let old_end = old.len() - suffix;
    let new_end = new.len() - suffix;
    InputEdit {
        start_byte: prefix,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: point(old, prefix),
        old_end_position: point(old, old_end),
        new_end_position: point(new, new_end),
    }
}

#[cfg(test)]
#[path = "../tests/fixtures.rs"]
#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod tests {
    use super::*;
    fn paint(colors: &mut [u8], patch: &Patch) {
        colors[patch.start..patch.start + patch.length].fill(0);
        for span in &patch.spans {
            colors[span.start..span.start + span.length].fill(span.style);
        }
    }
    pub(super) fn check(language: &str, sources: &[String]) {
        let mut document = Document::new(language).unwrap();
        let mut old = String::new();
        let mut colors = Vec::new();
        let mut revision = 0;
        for source in sources {
            let edit = source_edit(&old, source);
            let start = old[..edit.start_byte].encode_utf16().count();
            let end = old[..edit.old_end_byte].encode_utf16().count();
            let inserted = source[edit.start_byte..edit.new_end_byte]
                .encode_utf16()
                .count();
            colors.splice(start..end, std::iter::repeat_n(0, inserted));
            let patch = document.update(source, revision, false).unwrap();
            revision = patch.revision;
            paint(&mut colors, &patch);
            let reference = Document::new(language)
                .unwrap()
                .update(source, 0, true)
                .unwrap();
            let mut expected = vec![0; source.encode_utf16().count()];
            paint(&mut expected, &reference);
            assert_eq!(
                colors, expected,
                "incremental colors differ for {language}: {source}"
            );
            old = source.clone();
        }
    }
    #[test]
    fn all_languages_load_and_incremental_edits_match_fresh_colors() {
        for &language in languages::LANGUAGES {
            let source = fixtures::fixture(language, 12);
            let mut document =
                Document::new(language).unwrap_or_else(|e| panic!("{language}: {e}"));
            let full = document.update(&source, 0, false).unwrap();
            assert!(
                !document.tree.as_ref().unwrap().root_node().has_error(),
                "invalid fixture for {language}: {}",
                document.tree.as_ref().unwrap().root_node().to_sexp()
            );
            assert!(!full.spans.is_empty(), "no colors for {language}");
            let changed = fixtures::local_edit(language, &source, 12);
            let mut sources = vec![source.clone(), changed.clone(), source.clone()];
            // Undo/redo, broad deletions, malformed/partial syntax and recovery.
            sources.push(changed.replace("Żółć", "é 東京"));
            let middle = source
                .char_indices()
                .find(|(byte, _)| *byte >= source.len() / 2)
                .unwrap()
                .0;
            sources.push(source[..middle].to_owned());
            sources.push(source.clone());
            sources.push(String::new());
            sources.push(source);
            check(language, &sources);
        }
    }
    #[test]
    fn every_language_keeps_local_queries_and_patches_small() {
        for &language in languages::LANGUAGES {
            let source = fixtures::fixture(language, 200);
            let changed = fixtures::local_edit(language, &source, 200);
            let mut document = Document::new(language).unwrap();
            let revision = document.update(&source, 0, false).unwrap().revision;
            let patch = document.update(&changed, revision, false).unwrap();
            assert!(
                !patch.full
                    && patch.queried_bytes < 500
                    && patch.length < 500
                    && patch.spans.len() < 50,
                "lost locality for {language}: query={}, patch={}, spans={}",
                patch.queried_bytes,
                patch.length,
                patch.spans.len()
            );
            check(language, &[source, changed]);
        }
    }
    #[test]
    fn every_language_typing_and_deletion_preserves_colors() {
        for &language in languages::LANGUAGES {
            let source = fixtures::fixture(language, 1);
            let ends: Vec<_> = source
                .char_indices()
                .map(|(byte, _)| byte)
                .chain(std::iter::once(source.len()))
                .collect();
            let sources = ends
                .iter()
                .chain(ends.iter().rev())
                .map(|&end| source[..end].to_owned())
                .collect::<Vec<_>>();
            check(language, &sources);
        }
    }
    #[test]
    fn typing_deleting_unicode_crlf_and_invalid_states_match_full_coloring() {
        for (language, source) in [
            (
                "json",
                "{\r\n\"🙂\": [false, null, 42], \"Żółć\": \"é\\\"\"\r\n}",
            ),
            (
                "php",
                "<?php\r\n/* a\r\n b */\r\n$x = <<<TXT\r\n🙂 {$name}\r\nTXT;\r\necho $x; ?>",
            ),
        ] {
            let ends: Vec<_> = source
                .char_indices()
                .map(|(byte, _)| byte)
                .chain(std::iter::once(source.len()))
                .collect();
            let sources = ends
                .iter()
                .chain(ends.iter().rev())
                .map(|&end| source[..end].to_owned())
                .collect::<Vec<_>>();
            check(language, &sources);
        }
    }
    #[test]
    fn multiline_comments_strings_heredoc_and_far_apart_changes() {
        for (language, sources) in [
            (
                "php",
                vec![
                    "<?php\n$a=1;\n/* comment */\n$b='🙂';\necho $b;",
                    "<?php\n$a=1;\n/* comment \n$b='🙂';\necho $b;",
                    "<?php\n$a=12345;\n/* comment */\n$b='Łódź';\necho 42;",
                    "<?php\n$x=<<<TXT\nŁódź {$a}\nTXT;\necho 42;",
                    "<?php\n$x=<<<TXT\nŁódź {$a}\nBROKEN;\necho 42;",
                    "<?php\n$x=<<<'TXT'\n🙂 {$a}\nTXT;\necho 42;",
                    "<p><?php echo '🙂'; ?></p>",
                ],
            ),
            (
                "json",
                vec![
                    "{\"a\":1,\"b\":true,\"c\":\"🙂\"}",
                    "{\"a\":1,\"b\":false,\"c\":\"🙂\"}",
                    "{\"a\":12345,\"b\":false,\"c\":\"é Łódź\"}",
                    "[1, {\"a\":\"\\\"\"}, 3]",
                    "[1, 2, 3]",
                    "[]",
                    "",
                ],
            ),
        ] {
            check(
                language,
                &sources.into_iter().map(String::from).collect::<Vec<_>>(),
            );
        }
    }
    #[test]
    fn random_valid_edits_match_full_coloring() {
        for language in ["json", "php"] {
            let mut rows: Vec<_> = (0..100).map(|n| format!("value {n} 🙂")).collect();
            let mut sources = Vec::new();
            let mut random = 7u64;
            for step in 0..60 {
                random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                let index = random as usize % rows.len();
                rows[index] = format!("Łódź {step} é");
                if step % 9 == 0 {
                    rows.insert(index, "insert 🙂".into());
                }
                if step % 11 == 0 {
                    rows.remove(index);
                }
                sources.push(if language == "json" {
                    format!(
                        "{{{}}}",
                        rows.iter()
                            .enumerate()
                            .map(|(n, s)| format!("\"key{n}\":\"{s}\""))
                            .collect::<Vec<_>>()
                            .join(",\r\n")
                    )
                } else {
                    format!(
                        "<?php\r\n{}",
                        rows.iter()
                            .enumerate()
                            .map(|(n, s)| format!("$v{n}=\"{s}\";\r\n"))
                            .collect::<String>()
                    )
                });
            }
            check(language, &sources);
        }
    }
    #[test]
    fn nested_scopes_and_context_sensitive_captures_match_full_coloring() {
        for (language, original) in [
            (
                "json",
                r#"{"outer":[{"name":"🙂","values":[true,false,null]},{"other":{"count":42}}],"tail":"end"}"#,
            ),
            (
                "php",
                "<?php\nnamespace Example;\nclass Demo extends Base {\n/** comment */\npublic function run(int $n): string {\nif ($n > 0) { $value = \"🙂 {$n}\"; echo $value; }\nreturn 'tail';\n}\n}\nDemo::run(42);",
            ),
        ] {
            let mut sources = vec![original.to_owned()];
            for (from, to) in [
                ("🙂", "Łódź é"),
                ("true", "false"),
                ("42", "123456"),
                ("run", "execute"),
                ("Base", "Other"),
                ("$n", "$longName"),
                ("tail", "end🙂"),
            ] {
                let edited = sources.last().unwrap().replacen(from, to, 1);
                sources.push(edited);
            }
            // Inserting/removing scope boundaries and deleting syntax makes the
            // dirty region grow beyond the immediate edit, including recovery.
            sources.push(original.replace('}', ""));
            sources.push(original.to_owned());
            check(language, &sources);
        }
    }
    #[test]
    fn local_edit_only_queries_and_returns_small_region() {
        let source = format!(
            "{{{}}}",
            (0..10_000)
                .map(|n| format!("\"key{n}\":{{\"value\":true,\"name\":\"🙂\"}}"))
                .collect::<Vec<_>>()
                .join(",\n")
        );
        let mut document = Document::new("json").unwrap();
        let initial = document.update(&source, 0, false).unwrap();
        let edited = source.replacen(
            "\"key5000\":{\"value\":true",
            "\"key5000\":{\"value\":false",
            1,
        );
        let patch = document.update(&edited, initial.revision, false).unwrap();
        assert!(!patch.full);
        assert!(patch.queried_bytes < 500);
        assert!(patch.spans.len() < 50);
        assert!(
            patch.length < 500,
            "must not send all document colors to the UI"
        );
        let same = document.update(&edited, patch.revision, false).unwrap();
        assert_eq!(same.length, 0);
        assert!(same.spans.is_empty());
        let php = format!(
            "<?php\nfunction example() {{\n{}\n}}",
            (0..10_000)
                .map(|n| format!("$v{n}=true;\n"))
                .collect::<String>()
        );
        let mut document = Document::new("php").unwrap();
        let initial = document.update(&php, 0, false).unwrap();
        let edited = php.replace("$v5000=true", "$v5000=false");
        let patch = document.update(&edited, initial.revision, false).unwrap();
        assert!(!patch.full);
        assert!(patch.queried_bytes < 500);
        assert!(patch.length < 500);
        check("php", &[php, edited]);
    }
    #[test]
    fn dropped_revision_and_theme_refresh_return_full_cached_snapshot() {
        let mut document = Document::new("json").unwrap();
        let initial = document.update("{\"a\":true,\"b\":42}", 0, false).unwrap();
        document
            .update("{\"a\":false,\"b\":42}", initial.revision, false)
            .unwrap();
        let recovery = document
            .update("{\"a\":false,\"b\":43}", initial.revision, false)
            .unwrap();
        assert!(recovery.full);
        let theme = document
            .update("{\"a\":false,\"b\":43}", recovery.revision, true)
            .unwrap();
        assert!(theme.full);
        assert_eq!(theme.queried_bytes, 0);
        let reference = Document::new("json")
            .unwrap()
            .update("{\"a\":false,\"b\":43}", 0, true)
            .unwrap();
        assert_eq!(theme.spans, reference.spans);
    }
}

/// Shared palette category mapping for consumers and benchmark adapters.
pub fn style_category(name: &str) -> u8 {
    style(name).0
}

#[cfg(test)]
mod embedded_tests {
    use super::*;
    fn colors(language: &str, source: &str) -> Vec<u8> {
        let patch = Document::new(language)
            .unwrap()
            .update(source, 0, true)
            .unwrap();
        let mut result = vec![0; source.encode_utf16().count()];
        for span in patch.spans {
            result[span.start..span.start + span.length].fill(span.style);
        }
        result
    }
    fn token(language: &str, source: &str, needle: &str, expected: u8) {
        let index = source.find(needle).unwrap();
        assert_eq!(
            colors(language, source)[source[..index].encode_utf16().count()],
            expected,
            "{language}: {needle} in {source}"
        );
    }
    #[test]
    fn embedded_tokens_have_their_language_colors() {
        let html = "<p>Żółć 🙂</p><script>const name = '🙂'; // comment\n</script><style>/* css */ .x { color: red; }</style>";
        token("html", html, "const", 1);
        token("html", html, "// comment", 6);
        token("html", html, "/* css */", 6);
        token(
            "php",
            &format!("<div><?php echo '🙂'; ?></div>{html}"),
            "const",
            1,
        );
        token(
            "php",
            "<div class=\"<?php echo '🙂'; ?>\">ok</div>",
            "echo",
            1,
        );
        token(
            "html",
            "<script type=\"application/json\">{\"key\": true}</script>",
            "true",
            3,
        );
        token(
            "html",
            "<script type=\"text/typescript\">interface Entry { name: string }</script>",
            "interface",
            1,
        );
        token(
            "markdown",
            "```py\r\n# Żółć 🙂\r\nprint('🙂')\r\n```\r\n",
            "# Żółć",
            6,
        );
        token(
            "markdown",
            "~~~html\n<script>const value = '🙂';</script>\n~~~\n",
            "const",
            1,
        );
        token(
            "javascript",
            "const value = html`<script>const x = 42;</script><p>${name}</p>`;",
            "script>",
            1,
        );
        token(
            "javascript",
            "const value = sql`SELECT name FROM users WHERE id = ${id}`;",
            "SELECT",
            1,
        );
        token(
            "php",
            "<?php\n$sql = <<<'SQL'\nSELECT name FROM users;\nSQL;\n",
            "SELECT",
            1,
        );
        token(
            "dockerfile",
            "FROM alpine\nRUN if true; then echo '🙂'; fi\n",
            "if true",
            1,
        );
        token(
            "makefile",
            "all:\n\tif true; then echo '🙂'; fi\n",
            "if true",
            1,
        );
        // Unknown fences retain markup.raw; unknown MIME/tag does not become JS.
        token("markdown", "```unknown\nconst x = 1;\n```\n", "const", 2);
        token(
            "html",
            "<script type=\"text/plain\">const x = 1;</script>",
            "const",
            0,
        );
    }
    #[test]
    fn every_supported_language_can_be_embedded_in_markdown() {
        for language in languages::LANGUAGES {
            let inner = fixtures::fixture(language, 2);
            let outer = format!("# Żółć 🙂\r\n```{language}\r\n{inner}\r\n```\r\n");
            let actual = colors("markdown", &outer);
            let offset = outer[..outer.find(&inner).unwrap()].encode_utf16().count();
            assert_eq!(
                &actual[offset..offset + inner.encode_utf16().count()],
                colors(language, &inner),
                "{language}"
            );
        }
    }
    #[test]
    fn embedded_boundaries_typing_deletion_and_retagging_match_full_colors() {
        let samples = [
            (
                "html",
                "<script type=\"text/javascript\">const x = 'Żółć 🙂';\n</script><style>.x { color:red; }</style>",
            ),
            (
                "php",
                "<div><?php echo 'Żółć 🙂'; ?></div><script>const x = 42;</script>",
            ),
            (
                "markdown",
                "# Żółć 🙂\r\n```html\r\n<script>const x = 42;</script>\r\n```\r\n~~~py\r\n# comment\r\n~~~",
            ),
            (
                "javascript",
                "const a = sql`SELECT name FROM users WHERE id = ${id}`; const b = html`<p>Żółć 🙂</p>`;",
            ),
            (
                "php",
                "<?php\n$sql = <<<'SQL'\nSELECT 'Żółć 🙂' FROM users;\nSQL;\n",
            ),
        ];
        for (language, sample) in samples {
            let mut sources: Vec<String> = sample
                .char_indices()
                .map(|(index, _)| sample[..index].to_owned())
                .collect();
            sources.push(sample.into());
            sources.extend(sources.clone().into_iter().rev());
            sources.push(
                sample
                    .replace("html", "unknown")
                    .replace("javascript", "json")
                    .replace("py", "ruby"),
            );
            sources.push(sample.replace("Żółć", "Łódź é🙂").replace("</script>", ""));
            sources.push(sample.into());
            tests::check(language, &sources);
        }
    }
    #[test]
    fn combined_fragments_holes_aliases_and_layer_reordering_preserve_colors() {
        let php = "<script>const a = 'Żółć 🙂';<?php echo 'Łódź 🙂'; ?>const b = 42;</script><p><?= $name ?></p>";
        token("php", php, "const b", 1);
        token("php", php, "echo", 1);
        let js = "const a = html`<p>${name}</p>`; const b = sql`SELECT '🙂'`; const c = html`<span>Żółć</span>`;";
        token("javascript", js, "name}", 0);
        let heredoc = "<?php\n$query = <<<SQL\nSELECT '$value' AS title, '$this' AS owner;\nSQL;\n";
        token("php", heredoc, "SELECT", 1);
        token("php", heredoc, "$value", 0);
        token("php", heredoc, "$this", 0);
        tests::check(
            "php",
            &[
                heredoc.into(),
                heredoc.replace("$value", "$Łódź🙂"),
                heredoc.replace("$this", "$value"),
                heredoc.into(),
            ],
        );

        token("javascript", js, "SELECT", 1);
        tests::check(
            "php",
            &[
                php.into(),
                php.replace("Łódź 🙂", "組 é🙂🙂"),
                php.replace("<?php echo 'Łódź 🙂'; ?>", ""),
                php.replace("const b", "// const b"),
                php.into(),
            ],
        );
        tests::check(
            "javascript",
            &[
                js.into(),
                js.replace("const a = html`<p>${name}</p>`; ", ""),
                js.replace("sql`", "html`"),
                js.replace("html`", "unknown`"),
                js.replace("${name}", "${'Żółć 🙂'}"),
                js.into(),
            ],
        );
        token(
            "html",
            "<script TYPE=application/ld+json>{\"name\": true}</script>",
            "true",
            3,
        );
        token(
            "markdown",
            "```{.python} title='example'\n# comment\n```\n",
            "# comment",
            6,
        );
        token(
            "html",
            "<SCRIPT>const value = 42;</SCRIPT><STYLE>/* comment */ .x { color: red; }</STYLE>",
            "const",
            1,
        );
        // Excessive self-nesting is bounded without failing to render the host.
        let mut nested = "print('Żółć 🙂')\n".to_owned();
        for level in 3..16 {
            let fence = "~".repeat(level);
            nested = format!("{fence}markdown\n{nested}{fence}\n");
        }
        tests::check(
            "markdown",
            &[nested.clone(), nested.replace("Żółć", "Łódź é🙂")],
        );
    }
    #[test]
    fn all_embedded_workloads_match_full_colors_and_keep_edits_local() {
        for language in languages::LANGUAGES {
            for scenario in fixtures::embedded_scenarios(language) {
                let source = fixtures::embedded_fixture(language, 200, scenario);
                let changed = fixtures::local_edit("html", &source, 200);
                tests::check(
                    language,
                    &[
                        source.clone(),
                        changed.clone(),
                        source.clone(),
                        String::new(),
                        changed.clone(),
                    ],
                );
                let mut document = Document::new(language).unwrap();
                let first = document.update(&source, 0, false).unwrap();
                let next = document.update(&changed, first.revision, false).unwrap();
                assert!(!next.full, "{language}/{scenario}");
                assert!(
                    next.queried_bytes < 500,
                    "{language}/{scenario}: {}",
                    next.queried_bytes
                );
                assert!(next.length < 500, "{language}/{scenario}: {}", next.length);
            }
        }
    }
    #[test]
    fn large_embedded_edits_are_local_and_resync_is_cached() {
        for (language, inner) in [
            ("html", "javascript"),
            ("markdown", "python"),
            ("php", "javascript"),
        ] {
            let body = fixtures::fixture(inner, 10_000);
            let source = match language {
                "markdown" => format!("```{inner}\n{body}\n```\n"),
                "php" => format!("<?php echo 'header'; ?><script>{body}</script>"),
                _ => format!("<script>{body}</script>"),
            };
            let changed = fixtures::local_edit(inner, &source, 10_000);
            let mut document = Document::new(language).unwrap();
            let first = document.update(&source, 0, false).unwrap();
            let next = document.update(&changed, first.revision, false).unwrap();
            assert!(!next.full, "{language}");
            assert!(
                next.queried_bytes < 1000,
                "{language}: {}",
                next.queried_bytes
            );
            assert!(next.length < 1000, "{language}: {}", next.length);
            tests::check(language, &[source, changed.clone()]);
            let resync = document.update(&changed, 0, true).unwrap();
            assert!(resync.full);
            assert_eq!(resync.queried_bytes, 0);
        }
    }
}
