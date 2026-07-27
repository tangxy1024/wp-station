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
  "for"
  "then"
  "not"
  "within"
  "expect"
] @keyword

[
  "hit"
  "near_miss"
  "miss"
  "precision"
  "recall"
  "fpr"
  "latency_p95"
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
(percent) @number
(rate_constant) @number
(boolean) @constant.builtin
(comparison_operator) @operator

[ "(" ")" "{" "}" "<" ">" ] @punctuation.bracket
[ "," "=" ] @punctuation.delimiter
"#[" @attribute
".." @operator

(scenario_declaration name: (identifier) @function.definition)
(annotation_item key: (identifier) @attribute)
(stream_statement stream: (identifier) @type)
(injection_case rule: (identifier) @function)
(injection_case stream: (identifier) @type)
(sequence_block entity: (identifier) @variable.parameter)
(predicate key: (identifier) @property)
(expect_statement rule: (identifier) @function)
(wave_shape) @constant
(mode_keyword) @constant
(expect_function) @function.builtin

(identifier) @variable
