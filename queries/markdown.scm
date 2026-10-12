[(atx_heading) (setext_heading)] @markup.heading
[(code_span) (fenced_code_block) (indented_code_block)] @markup.raw
(strong_emphasis) @markup.strong
(emphasis) @markup.italic
[(link_text) (link_label) (image_description)] @string.special.key
[(link_destination) (link_title) (uri_autolink) (email_autolink)] @string
(html_comment) @comment
(backslash_escape) @escape
