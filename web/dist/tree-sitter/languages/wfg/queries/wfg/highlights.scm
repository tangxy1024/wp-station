; WFG Syntax Highlighting (tree-sitter native)

[
  "use"
  "scenario"
  "traffic"
  "stream"
  "gen"
  "wave"
  "burst"
  "timeline"
  "injection"
  "seq"
  "with"
  "expect"
] @keyword

[
  "hit"
  "near_miss"
  "miss"
] @keyword

[
  "base"
  "amp"
  "period"
  "shape"
  "peak"
  "every"
  "hold"
] @property

(comment) @comment
(string) @string
(number) @number
(duration) @number
(rate_constant) @number
(percent) @number
(boolean) @constant.builtin
(wave_shape) @constant.builtin

(scenario_declaration name: (identifier) @function.definition)
(annotation_item key: (identifier) @property)
(stream_statement stream: (identifier) @variable)
(injection_case stream: (identifier) @variable)
(sequence_block entity: (identifier) @variable)
(predicate key: (identifier) @property)
(expect_statement rule: (identifier) @function)

(identifier) @variable
