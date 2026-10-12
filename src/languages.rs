use std::borrow::Cow;
use tree_sitter::Language;

/// Supported editor languages. JSX/TSX use their appropriate query/grammar variants.
pub const LANGUAGES: &[&str] = &[
    "bash",
    "c",
    "cpp",
    "csharp",
    "css",
    "diff",
    "dockerfile",
    "go",
    "html",
    "ini",
    "java",
    "javascript",
    "json",
    "jsx",
    "kotlin",
    "makefile",
    "markdown",
    "nginx",
    "php",
    "python",
    "ruby",
    "rust",
    "scss",
    "sql",
    "swift",
    "toml",
    "tsx",
    "typescript",
    "xml",
    "yaml",
];

pub struct Grammar {
    pub language: Language,
    pub highlights: Cow<'static, str>,
    pub version: &'static str,
}

/// Also used by the comparison tool so both engines always get identical queries.
pub fn grammar(name: &str) -> Result<Grammar, String> {
    let (language, highlights, version): (Language, Cow<'static, str>, &str) = match name {
        "bash" => (
            tree_sitter_bash::LANGUAGE.into(),
            tree_sitter_bash::HIGHLIGHT_QUERY.into(),
            "0.25.1",
        ),
        "c" => (
            tree_sitter_c::LANGUAGE.into(),
            tree_sitter_c::HIGHLIGHT_QUERY.into(),
            "0.24.2",
        ),
        "cpp" => (
            tree_sitter_cpp::LANGUAGE.into(),
            format!(
                "{}\n{}",
                tree_sitter_c::HIGHLIGHT_QUERY,
                tree_sitter_cpp::HIGHLIGHT_QUERY
            )
            .into(),
            "0.23.4",
        ),
        "csharp" => (
            tree_sitter_c_sharp::LANGUAGE.into(),
            tree_sitter_c_sharp::HIGHLIGHTS_QUERY.into(),
            "0.23.5",
        ),
        "css" => (
            tree_sitter_css::LANGUAGE.into(),
            tree_sitter_css::HIGHLIGHTS_QUERY.into(),
            "0.25.0",
        ),
        "diff" => (
            tree_sitter_diff::LANGUAGE.into(),
            format!(
                "{}\n(addition) @diff.plus\n(deletion) @diff.minus",
                tree_sitter_diff::HIGHLIGHTS_QUERY
            )
            .into(),
            "0.1.0",
        ),
        "dockerfile" => (
            tree_sitter_dockerfile_updated::language(),
            include_str!("../queries/dockerfile.scm").into(),
            "0.2.0",
        ),
        "go" => (
            tree_sitter_go::LANGUAGE.into(),
            tree_sitter_go::HIGHLIGHTS_QUERY.into(),
            "0.25.0",
        ),
        "html" => (
            tree_sitter_html::LANGUAGE.into(),
            tree_sitter_html::HIGHLIGHTS_QUERY.into(),
            "0.23.2",
        ),
        "ini" => (
            tree_sitter_ini::LANGUAGE.into(),
            tree_sitter_ini::HIGHLIGHTS_QUERY.into(),
            "1.4.0",
        ),
        "java" => (
            tree_sitter_java::LANGUAGE.into(),
            tree_sitter_java::HIGHLIGHTS_QUERY.into(),
            "0.23.5",
        ),
        "javascript" | "jsx" => (
            tree_sitter_javascript::LANGUAGE.into(),
            format!(
                "{}\n{}",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                tree_sitter_javascript::JSX_HIGHLIGHT_QUERY
            )
            .into(),
            "0.25.0",
        ),
        "json" => (
            tree_sitter_json::LANGUAGE.into(),
            tree_sitter_json::HIGHLIGHTS_QUERY.into(),
            "0.24.8",
        ),
        "kotlin" => (
            tree_sitter_kotlin_ng::LANGUAGE.into(),
            include_str!("../queries/kotlin.scm").into(),
            "1.1.0",
        ),
        "makefile" => (
            tree_sitter_make::LANGUAGE.into(),
            tree_sitter_make::HIGHLIGHTS_QUERY.into(),
            "1.1.1",
        ),
        "markdown" => (
            tree_sitter_markdown_fork::language(),
            include_str!("../queries/markdown.scm").into(),
            "0.7.3",
        ),
        "nginx" => (
            tree_sitter_nginx::LANGUAGE.into(),
            include_str!("../queries/nginx.scm").into(),
            "1.0.1",
        ),
        "php" => (
            tree_sitter_php::LANGUAGE_PHP.into(),
            tree_sitter_php::HIGHLIGHTS_QUERY.into(),
            "0.24.2",
        ),
        "python" => (
            tree_sitter_python::LANGUAGE.into(),
            tree_sitter_python::HIGHLIGHTS_QUERY.into(),
            "0.25.0",
        ),
        "ruby" => (
            tree_sitter_ruby::LANGUAGE.into(),
            tree_sitter_ruby::HIGHLIGHTS_QUERY.into(),
            "0.23.1",
        ),
        "rust" => (
            tree_sitter_rust::LANGUAGE.into(),
            tree_sitter_rust::HIGHLIGHTS_QUERY.into(),
            "0.24.2",
        ),
        "scss" => (
            tree_sitter_scss::language(),
            format!(
                "{}\n{}",
                tree_sitter_css::HIGHLIGHTS_QUERY,
                tree_sitter_scss::HIGHLIGHTS_QUERY
            )
            .into(),
            "1.0.0",
        ),
        "sql" => (
            tree_sitter_sequel::LANGUAGE.into(),
            tree_sitter_sequel::HIGHLIGHTS_QUERY.into(),
            "0.3.11",
        ),
        "swift" => (
            tree_sitter_swift::LANGUAGE.into(),
            tree_sitter_swift::HIGHLIGHTS_QUERY.into(),
            "0.7.4",
        ),
        "toml" => (
            tree_sitter_toml_ng::LANGUAGE.into(),
            tree_sitter_toml_ng::HIGHLIGHTS_QUERY.into(),
            "0.7.0",
        ),
        "typescript" | "tsx" => (
            if name == "tsx" {
                tree_sitter_typescript::LANGUAGE_TSX.into()
            } else {
                tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
            },
            format!(
                "{}\n{}\n{}",
                tree_sitter_javascript::HIGHLIGHT_QUERY,
                if name == "tsx" {
                    tree_sitter_javascript::JSX_HIGHLIGHT_QUERY
                } else {
                    ""
                },
                tree_sitter_typescript::HIGHLIGHTS_QUERY
            )
            .into(),
            "0.23.2",
        ),
        "xml" => (
            tree_sitter_xml::LANGUAGE_XML.into(),
            tree_sitter_xml::XML_HIGHLIGHT_QUERY.into(),
            "0.7.0",
        ),
        "yaml" => (
            tree_sitter_yaml::LANGUAGE.into(),
            tree_sitter_yaml::HIGHLIGHTS_QUERY.into(),
            "0.7.2",
        ),
        _ => return Err(format!("Unsupported syntax language: {name}")),
    };
    Ok(Grammar {
        language,
        highlights,
        version,
    })
}

/// Injection queries shared with the upstream comparison engine.
pub fn injections(name: &str) -> &'static str {
    match name {
        "html" => include_str!("../queries/injections/html.scm"),
        "php" => include_str!("../queries/injections/php.scm"),
        "markdown" => include_str!("../queries/injections/markdown.scm"),
        "javascript" | "jsx" | "typescript" | "tsx" => {
            include_str!("../queries/injections/javascript.scm")
        }
        "dockerfile" => include_str!("../queries/injections/dockerfile.scm"),
        "makefile" => include_str!("../queries/injections/makefile.scm"),
        _ => "",
    }
}

/// Fence/tag/MIME aliases. Unknown languages keep their host coloring.
pub fn resolve_injection(name: &str) -> Option<&'static str> {
    let name = name
        .split_whitespace()
        .next()?
        .trim_matches(|c| c == '{' || c == '}' || c == '.')
        .to_ascii_lowercase();
    let canonical = match name.as_str() {
        "js"
        | "node"
        | "module"
        | "text/javascript"
        | "application/javascript"
        | "application/x-javascript"
        | "text/ecmascript" => "javascript",
        "text/jsx" => "jsx",
        "text/tsx" => "tsx",
        "ts" | "text/typescript" | "application/typescript" => "typescript",
        "application/json" | "application/ld+json" => "json",
        "text/html" => "html",
        "text/css" => "css",
        "sh" | "shell" | "shellscript" | "zsh" => "bash",
        "py" | "python3" => "python",
        "rb" => "ruby",
        "rs" => "rust",
        "yml" => "yaml",
        "c++" | "cxx" => "cpp",
        "cs" | "c#" => "csharp",
        "md" => "markdown",
        "docker" => "dockerfile",
        "make" => "makefile",
        "htm" => "html",
        "postgresql" | "pgsql" => "sql",
        other => other,
    };
    LANGUAGES
        .iter()
        .copied()
        .find(|language| *language == canonical)
}
