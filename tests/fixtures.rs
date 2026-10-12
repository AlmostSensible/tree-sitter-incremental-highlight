//! Shared deterministic workloads for regression tests and the standalone benchmark.
pub fn fixture(language: &str, entries: usize) -> String {
    if language == "json" {
        return format!(
            "{{\r\n{}\r\n}}\r\n",
            (0..entries)
                .map(|n| format!(
                    "  \"entry_{n}\": {{\"name\": \"Żółć 🙂\", \"enabled\": true, \"count\": {n}}}"
                ))
                .collect::<Vec<_>>()
                .join(",\r\n")
        );
    }
    let body: String = (0..entries)
        .map(|n| match language {
            "bash" => format!("value_{n}=\"Żółć 🙂\" # comment\r\n"),
            "c" | "cpp" => format!("const char *value_{n} = \"Żółć 🙂\"; // comment\r\n"),
            "csharp" => format!("public string value_{n} = \"Żółć 🙂\"; // comment\r\n"),
            "css" | "scss" => format!(".entry_{n} {{ content: \"Żółć 🙂\"; color: #fff; }}\r\n"),
            "diff" => format!("-old value_{n}\r\n+Żółć 🙂 value_{n}\r\n"),
            "dockerfile" => format!("ENV VALUE_{n}=\"Żółć 🙂\"\r\n"),
            "go" => format!("var value_{n} = \"Żółć 🙂\" // comment\r\n"),
            "html" | "xml" => format!("<entry id=\"value_{n}\" title=\"Żółć 🙂\">text</entry>\r\n"),
            "ini" | "toml" => format!("[entry_{n}]\r\nname = \"Żółć 🙂\"\r\n"),
            "java" => format!("public static final String value_{n} = \"Żółć 🙂\"; // comment\r\n"),
            "javascript" => format!("const value_{n} = \"Żółć 🙂\"; // comment\r\n"),
            "jsx" => format!("const value_{n} = <span title=\"Żółć 🙂\">text</span>;\r\n"),
            "kotlin" => format!("val value_{n} = \"Żółć 🙂\" // comment\r\n"),
            "makefile" => format!("value_{n} := \"Żółć 🙂\" # comment\r\n"),
            "markdown" => format!(
                "## Entry {n}\r\n\r\n**Żółć 🙂** and [link](https://example.invalid/{n})\r\n\r\n"
            ),
            "nginx" => format!("set $value_{n} \"Żółć 🙂\"; # comment\r\n"),
            "php" => format!("$value_{n} = \"Żółć 🙂\"; // comment\r\n"),
            "python" => format!("value_{n} = \"Żółć 🙂\" # comment\r\n"),
            "ruby" => format!("value_{n} = \"Żółć 🙂\" # comment\r\n"),
            "rust" => format!("const VALUE_{n}: &str = \"Żółć 🙂\"; // comment\r\n"),
            "sql" => format!("SELECT 'Żółć 🙂' AS value_{n}; -- comment\r\n"),
            "swift" => format!("let value_{n} = \"Żółć 🙂\" // comment\r\n"),
            "typescript" => format!("const value_{n}: string = \"Żółć 🙂\"; // comment\r\n"),
            "tsx" => {
                format!("const value_{n}: JSX.Element = <span title=\"Żółć 🙂\">text</span>;\r\n")
            }
            "yaml" => format!("value_{n}: \"Żółć 🙂\" # comment\r\n"),
            _ => panic!("Missing benchmark fixture: {language}"),
        })
        .collect();
    match language {
        "csharp" | "java" => format!("class Demo {{\r\n{body}}}\r\n"),
        "dockerfile" => format!("FROM alpine\r\n{body}"),
        "go" => format!("package example\r\n{body}"),
        "html" | "xml" => format!("<root>\r\n{body}</root>\r\n"),
        "nginx" => format!("server {{\r\n{body}}}\r\n"),
        "php" => format!("<?php\r\n/* multiline\r\n comment */\r\n{body}"),
        "diff" => format!(
            "diff --git a/demo.txt b/demo.txt\r\n--- a/demo.txt\r\n+++ b/demo.txt\r\n@@ -1,{entries} +1,{entries} @@\r\n{body}"
        ),
        _ => body,
    }
}

pub fn local_edit(language: &str, source: &str, entries: usize) -> String {
    let n = entries / 2;
    if language == "json" {
        let needle = format!("\"entry_{n}\": {{\"name\": \"Żółć 🙂\", \"enabled\": true");
        return source.replacen(&needle, &needle.replace("true", "false"), 1);
    }
    if language == "php" {
        let needle = format!("$value_{n} = \"Żółć 🙂\"");
        return source.replacen(&needle, &needle.replace("Żółć", "Łódź🙂"), 1);
    }
    let middle = source
        .char_indices()
        .find(|(byte, _)| *byte >= source.len() / 2)
        .unwrap()
        .0;
    source[..middle].to_owned() + &source[middle..].replacen("Żółć", "Łódź🙂", 1)
}

/// Shared embedded workloads for the Rust engine and the native editor.
pub fn embedded_scenarios(language: &str) -> &'static [&'static str] {
    match language {
        "html" => &[
            "embedded-javascript",
            "embedded-css",
            "embedded-json",
            "embedded-typescript",
        ],
        "php" => &["embedded-html", "embedded-script", "embedded-heredoc"],
        "markdown" => &["embedded-python", "embedded-html", "embedded-unknown"],
        "javascript" | "typescript" | "tsx" => {
            &["embedded-sql", "embedded-html", "embedded-interpolation"]
        }
        _ => &[],
    }
}
pub fn embedded_fixture(language: &str, entries: usize, scenario: &str) -> String {
    let kind = scenario.strip_prefix("embedded-").unwrap();
    match (language, kind) {
        ("html", kind) => {
            let body = fixture(kind, entries);
            match kind {
                "css" => format!("<style>\r\n{body}</style>\r\n"),
                "json" => format!("<script type=\"application/ld+json\">\r\n{body}</script>\r\n"),
                "typescript" => format!("<script type=\"text/typescript\">\r\n{body}</script>\r\n"),
                _ => format!("<script>\r\n{body}</script>\r\n"),
            }
        }
        ("php", "html") => {
            let body: String = (0..entries)
                .map(|n| format!("<p title=\"Żółć 🙂\"><?php echo $value_{n}; ?></p>\r\n"))
                .collect();
            format!("<?php echo 'header'; ?>\r\n{body}")
        }
        ("php", "script") => format!(
            "<?php echo 'header'; ?><script>\r\n{}</script>\r\n",
            fixture("javascript", entries)
        ),
        ("php", "heredoc") => format!(
            "<?php\r\n$query = <<<'SQL'\r\n{}SQL;\r\n",
            fixture("sql", entries)
        ),
        ("markdown", "python") => {
            format!("# Demo\r\n```py\r\n{}```\r\n", fixture("python", entries))
        }
        ("markdown", "html") => format!(
            "# Demo\r\n~~~html\r\n<script>\r\n{}</script>\r\n~~~\r\n",
            fixture("javascript", entries)
        ),
        ("markdown", "unknown") => {
            format!("```unknown\r\n{}```\r\n", fixture("javascript", entries))
        }
        (_, "sql") => format!("const query = sql`\r\n{}`;\r\n", fixture("sql", entries)),
        (_, "html") => format!(
            "const template = html`\r\n{}`;\r\n",
            fixture("html", entries)
        ),
        (_, "interpolation") => {
            let body: String = (0..entries)
                .map(|n| format!("<p title=\"Żółć 🙂\">${{value_{n}}}</p>\r\n"))
                .collect();
            format!("const template = html`\r\n{body}`;\r\n")
        }
        _ => panic!("Missing embedded fixture {language}/{scenario}"),
    }
}
