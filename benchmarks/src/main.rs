use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap, env, fs, hint::black_box, path::Path, sync::OnceLock, time::Instant,
};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};
use tree_sitter_incremental_highlight::{Document, Patch, Span};

const SAMPLES: usize = 15;

#[path = "../../tests/fixtures.rs"]
mod fixtures;
use fixtures::fixture;
use tree_sitter_incremental_highlight::{languages, style_category};

fn global_names() -> &'static Vec<String> {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let mut names = Vec::new();
        for language in languages::LANGUAGES {
            let grammar = languages::grammar(language).unwrap();
            let query = tree_sitter::Query::new(&grammar.language, &grammar.highlights).unwrap();
            names.extend(query.capture_names().iter().map(|name| (*name).to_owned()));
        }
        names.sort();
        names.dedup();
        names
    })
}
fn configuration(language: &str) -> (HighlightConfiguration, Vec<u8>) {
    let grammar = languages::grammar(language).unwrap();
    let mut config = HighlightConfiguration::new(
        grammar.language,
        language,
        &grammar.highlights,
        languages::injections(language),
        "",
    )
    .unwrap();
    config.configure(global_names());
    (
        config,
        global_names()
            .iter()
            .map(|name| style_category(name))
            .collect(),
    )
}
fn injected_configuration(name: &str) -> Option<&'static HighlightConfiguration> {
    static CONFIGS: OnceLock<HashMap<String, HighlightConfiguration>> = OnceLock::new();
    let language = languages::resolve_injection(name)?;
    CONFIGS
        .get_or_init(|| {
            languages::LANGUAGES
                .iter()
                .map(|language| ((*language).into(), configuration(language).0))
                .collect()
        })
        .get(language)
}

fn upstream_spans(
    highlighter: &mut Highlighter,
    config: &HighlightConfiguration,
    styles: &[u8],
    text: &str,
) -> Vec<Span> {
    let mut offsets = vec![0; text.len() + 1];
    let mut length = 0;
    for (byte, c) in text.char_indices() {
        offsets[byte] = length;
        length += c.len_utf16();
    }
    offsets[text.len()] = length;
    let mut stack = Vec::new();
    let mut spans: Vec<Span> = Vec::new();
    for event in highlighter
        .highlight(config, text.as_bytes(), None, |name| {
            injected_configuration(name)
        })
        .unwrap()
    {
        match event.unwrap() {
            HighlightEvent::HighlightStart(highlight) => stack.push(styles[highlight.0]),
            HighlightEvent::HighlightEnd => {
                stack.pop().unwrap();
            }
            HighlightEvent::Source { start, end } => {
                let color = stack.last().copied().unwrap_or(0);
                if color == 0 || start == end {
                    continue;
                }
                let start = offsets[start];
                let end = offsets[end];
                if let Some(last) = spans
                    .last_mut()
                    .filter(|last| last.start + last.length == start && last.style == color)
                {
                    last.length += end - start;
                } else {
                    spans.push(Span {
                        start,
                        length: end - start,
                        style: color,
                    });
                }
            }
        }
    }
    spans
}

fn upstream_events(
    highlighter: &mut Highlighter,
    config: &HighlightConfiguration,
    text: &str,
) -> usize {
    let mut count = 0;
    for event in highlighter
        .highlight(config, text.as_bytes(), None, |name| {
            injected_configuration(name)
        })
        .unwrap()
    {
        black_box(event.unwrap());
        count += 1;
    }
    count
}
fn injection_audit(config: &HighlightConfiguration, text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut highlighter = Highlighter::new();
    for event in highlighter
        .highlight(config, text.as_bytes(), None, |name| {
            let config = injected_configuration(name);
            if let Some(language) = languages::resolve_injection(name) {
                names.push(language.to_owned());
            }
            config
        })
        .unwrap()
    {
        event.unwrap();
    }
    names
}
fn timed<T>(f: impl FnOnce() -> T) -> (f64, T) {
    let start = Instant::now();
    let result = black_box(f());
    (start.elapsed().as_secs_f64() * 1000.0, result)
}
fn paint(colors: &mut [u8], spans: &[Span]) {
    for span in spans {
        colors[span.start..span.start + span.length].fill(span.style);
    }
}
fn patch_colors(colors: &mut Vec<u8>, old: &str, new: &str, patch: &Patch) {
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
    let start = old[..prefix].encode_utf16().count();
    let end = old[..old.len() - suffix].encode_utf16().count();
    let inserted = new[prefix..new.len() - suffix].encode_utf16().count();
    colors.splice(start..end, std::iter::repeat_n(0, inserted));
    colors[patch.start..patch.start + patch.length].fill(0);
    paint(colors, &patch.spans);
}
fn median(values: &[f64]) -> f64 {
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
fn edited(language: &str, source: &str, entries: usize, scenario: &str) -> String {
    if scenario == "malformed" {
        return if language == "json" {
            source.replacen("\"entry_0\"", "\"entry_0", 1)
        } else {
            source.replacen("*/", "", 1)
        };
    }
    fixtures::local_edit(
        if scenario.starts_with("embedded-") {
            "html"
        } else {
            language
        },
        source,
        entries,
    )
}

fn benchmark(
    language: &str,
    entries: usize,
    scenario: &str,
    directory: &Path,
) -> serde_json::Value {
    let mut source = if scenario.starts_with("embedded-") {
        fixtures::embedded_fixture(language, entries, scenario)
    } else {
        fixture(language, entries)
    };
    if scenario == "function-local" {
        source = source.replacen("$value_0", "function example() {\r\n$value_0", 1) + "}\r\n";
    }
    let changed = edited(language, &source, entries, scenario);
    assert_ne!(source, changed);
    fs::write(
        directory.join(format!("{language}-{entries}-{scenario}.{language}")),
        &source,
    )
    .unwrap();
    let (own_setup, _) = timed(|| Document::new(language).unwrap());
    let (upstream_setup, (config, styles)) = timed(|| configuration(language));
    let resolved_injections = injection_audit(&config, &source);
    if scenario.starts_with("embedded-") {
        let expected: &[&str] = match scenario {
            "embedded-unknown" => &[],
            "embedded-heredoc" | "embedded-sql" => &["sql"],
            "embedded-script" => &["html", "javascript"],
            "embedded-python" => &["python"],
            "embedded-html" if language == "markdown" => &["html", "javascript"],
            "embedded-html" | "embedded-interpolation" => &["html"],
            "embedded-css" => &["css"],
            "embedded-json" => &["json"],
            "embedded-typescript" => &["typescript"],
            _ => &["javascript"],
        };
        assert_eq!(
            resolved_injections, expected,
            "Reference must actually parse every embedded language: {language}/{scenario}"
        );
    }
    let mut reference_highlighter = Highlighter::new();
    let mut style_differences = Vec::new();
    let mut upstream_expected = Vec::new();
    let expected: Vec<Vec<u8>> = [&source, &changed]
        .iter()
        .map(|text| {
            let spans = upstream_spans(&mut reference_highlighter, &config, &styles, text);
            let mut colors = vec![0; text.encode_utf16().count()];
            paint(&mut colors, &spans);
            let fresh = Document::new(language)
                .unwrap()
                .update(text, 0, true)
                .unwrap();
            let mut ours = vec![0; colors.len()];
            paint(&mut ours, &fresh.spans);
            let count = ours.iter().zip(&colors).filter(|(a, b)| a != b).count();
            let first = ours.iter().zip(&colors).position(|(a, b)| a != b);
            style_differences.push(json!({"utf16_positions":count,"first_utf16":first}));
            upstream_expected.push(colors);
            ours
        })
        .collect();

    let mut own_full = Vec::new();
    let mut upstream_full = Vec::new();
    let mut upstream_raw_full = Vec::new();
    let mut own_edit = Vec::new();
    let mut upstream_edit = Vec::new();
    let mut upstream_raw_edit = Vec::new();
    let mut query_bytes = Vec::new();
    let mut patch_lengths = Vec::new();
    let mut patch_spans = Vec::new();
    let mut full = Vec::new();
    let mut document = Document::new(language).unwrap();
    let mut revision = document.update(&source, 0, false).unwrap().revision;
    let mut previous = source.clone();
    let mut colors = expected[0].clone();
    let mut highlighter = Highlighter::new();
    let mut raw = Highlighter::new();
    let mut full_spans = 0;
    let mut events = 0;
    // Two untimed warmups; rotate order to avoid always favoring a warm CPU/cache.
    for step in 0..SAMPLES + 2 {
        let target_index = if step % 2 == 0 { 1 } else { 0 };
        let text = if target_index == 1 { &changed } else { &source };
        let mut own_ms = 0.0;
        let mut patch = None;
        let mut up_ms = 0.0;
        let mut raw_ms = 0.0;
        for offset in 0..3 {
            match (step + offset) % 3 {
                0 => {
                    let measured = timed(|| document.update(text, revision, false).unwrap());
                    own_ms = measured.0;
                    patch = Some(measured.1);
                }
                1 => {
                    let (ms, spans) =
                        timed(|| upstream_spans(&mut highlighter, &config, &styles, text));
                    up_ms = ms;
                    full_spans = spans.len();
                    let mut actual = vec![0; text.encode_utf16().count()];
                    paint(&mut actual, &spans);
                    assert!(
                        actual == upstream_expected[target_index],
                        "upstream reuse differs from fresh highlighting"
                    );
                }
                _ => {
                    let (ms, count) = timed(|| upstream_events(&mut raw, &config, text));
                    raw_ms = ms;
                    events = count;
                }
            }
        }
        let patch = patch.unwrap();
        revision = patch.revision;
        patch_colors(&mut colors, &previous, text, &patch);
        assert!(
            colors == expected[target_index],
            "{language}/{scenario}: incremental edit colors differ"
        );
        previous = text.clone();
        if step >= 2 {
            own_edit.push(own_ms);
            upstream_edit.push(up_ms);
            upstream_raw_edit.push(raw_ms);
            query_bytes.push(patch.queried_bytes);
            patch_lengths.push(patch.length);
            patch_spans.push(patch.spans.len());
            full.push(patch.full);
        }
        let mut own_load = 0.0;
        let mut upstream_load = 0.0;
        let mut raw_load = 0.0;
        for offset in 0..3 {
            match (step + offset) % 3 {
                0 => {
                    own_load = timed(|| {
                        Document::new(language)
                            .unwrap()
                            .update(&source, 0, false)
                            .unwrap()
                    })
                    .0;
                }
                1 => {
                    upstream_load = timed(|| {
                        upstream_spans(&mut Highlighter::new(), &config, &styles, &source)
                    })
                    .0;
                }
                _ => {
                    raw_load =
                        timed(|| upstream_events(&mut Highlighter::new(), &config, &source)).0;
                }
            }
        }
        if step >= 2 {
            own_full.push(own_load);
            upstream_full.push(upstream_load);
            upstream_raw_full.push(raw_load);
        }
    }
    println!(
        "{language}/{entries}/{scenario}: edit ours {:.2} ms, upstream UTF16 {:.2} ms ({:.2}×), raw events {:.2} ms; load ours {:.2} ms, upstream {:.2} ms",
        median(&own_edit),
        median(&upstream_edit),
        median(&upstream_edit) / median(&own_edit),
        median(&upstream_raw_edit),
        median(&own_full),
        median(&upstream_full)
    );
    json!({"language":language,"resolved_upstream_injections":resolved_injections,"grammar_version":languages::grammar(language).unwrap().version,"entries":entries,"scenario":scenario,"bytes":source.len(),"edited_bytes":changed.len(),
        "sha256":format!("{:x}",Sha256::digest(source.as_bytes())),"edited_sha256":format!("{:x}",Sha256::digest(changed.as_bytes())),
        "ours_setup_ms":own_setup,"upstream_setup_ms":upstream_setup,"ours_load_ms":own_full,"upstream_utf16_load_ms":upstream_full,
        "upstream_raw_load_ms":upstream_raw_full,"ours_edit_ms":own_edit,"upstream_utf16_edit_ms":upstream_edit,"upstream_raw_edit_ms":upstream_raw_edit,
        "queried_bytes":query_bytes,"patch_utf16":patch_lengths,"patch_spans":patch_spans,"full_patches":full,
        "upstream_full_spans":full_spans,"upstream_events":events,"cross_engine_style_differences":style_differences,
        "validation":"Every UTF16 color equals its engine's independent full result after all 17 sequential edits; scope-priority differences between engines recorded"})
}
fn main() {
    let path = env::args()
        .nth(1)
        .expect("usage: highlight-benchmark REPORT.json");
    let directory = Path::new(&path).parent().unwrap();
    fs::create_dir_all(directory).unwrap();
    if env::var_os("HIGHLIGHT_AUDIT_ONLY").is_some() {
        for language in languages::LANGUAGES {
            let (config, _) = configuration(language);
            for scenario in fixtures::embedded_scenarios(language) {
                let source = fixtures::embedded_fixture(language, 2, scenario);
                println!(
                    "{language}/{scenario}: {:?}",
                    injection_audit(&config, &source)
                );
            }
        }
        return;
    }
    // Compile the full injection registry outside timing. All configurations use
    // the same capture IDs, so nested HighlightStart events map to correct colors.
    injected_configuration("html").unwrap();
    let mut workloads = Vec::new();
    let requested = env::var("HIGHLIGHT_LANGUAGES").ok();
    let selected: Vec<&str> = requested
        .as_deref()
        .map(|names| names.split(',').collect())
        .unwrap_or_else(|| languages::LANGUAGES.to_vec());
    assert!(
        selected
            .iter()
            .all(|name| languages::LANGUAGES.contains(name)),
        "Unknown benchmark language"
    );
    let sizes: Vec<usize> = env::var("HIGHLIGHT_SIZES")
        .unwrap_or_else(|_| "1000,10000".into())
        .split(',')
        .map(|size| size.parse().expect("Invalid HIGHLIGHT_SIZES"))
        .collect();
    assert!(!sizes.is_empty() && sizes.iter().all(|size| *size > 0));
    for &language in &selected {
        for &entries in &sizes {
            workloads.push(benchmark(language, entries, "local", directory));
            for scenario in fixtures::embedded_scenarios(language) {
                workloads.push(benchmark(language, entries, scenario, directory));
            }
        }
        if ["json", "php"].contains(&language) {
            workloads.push(benchmark(language, 10_000, "malformed", directory));
        }
    }
    if selected.contains(&"php") {
        workloads.push(benchmark("php", 10_000, "function-local", directory));
    }
    let report = json!({"configuration":"Rust Release; 15 samples after 2 warmups; rotated order; identical grammars/queries/styles; injections enabled in BOTH engines; fully consumed events; no FFI/UI; source edits/oracle checks outside timing",
        "versions":{"tree-sitter":"0.25.10","tree-sitter-highlight":"0.25.10","tree-sitter-json":"0.24.8","tree-sitter-php":"0.24.2"},"selected_languages":selected,"workloads":workloads});
    fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}
