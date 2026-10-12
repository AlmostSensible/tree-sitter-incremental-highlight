[(line_comment) (block_comment)] @comment
[(string_literal) (multiline_string_literal) (character_literal)] @string
(escape_sequence) @escape
[(number_literal) (float_literal)] @number
(user_type) @type
(function_declaration (identifier) @function)
["class" "object" "fun" "val" "var" "if" "else" "when" "for" "while" "return" "import" "package" "try" "catch" "throw" "is" "as" "in"] @keyword
