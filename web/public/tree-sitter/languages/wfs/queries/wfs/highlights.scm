; WFS Syntax Highlighting (tree-sitter native)

[
  "window"
  "stream"
  "stream_tag"
  "time"
  "over"
  "fields"
] @keyword

(comment) @comment
(string) @string
(duration) @number

[
  "array"
  "chars"
  "digit"
  "float"
  "bool"
  "time"
  "ip"
  "hex"
] @type.builtin

(object_type) @type.builtin

(window_declaration name: (identifier) @function.definition)
(field_declaration name: (_) @property)
(field_declaration type: (_) @type)
(time_attribute (identifier) @property)
(quoted_identifier) @string.special

(identifier) @variable
