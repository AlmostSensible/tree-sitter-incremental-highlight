((text) @injection.content
 (#set! injection.language "html") (#set! injection.combined) (#set! injection.include-children))
(heredoc (heredoc_body (string_content) @injection.content) (heredoc_end) @injection.language (#set! injection.combined) (#set! injection.include-children))
(nowdoc (nowdoc_body) @injection.content (heredoc_end) @injection.language (#set! injection.combined) (#set! injection.include-children))
