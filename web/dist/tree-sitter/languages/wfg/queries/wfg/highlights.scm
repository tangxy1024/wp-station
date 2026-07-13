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
(boolean) @constant.builtin

(scenario_declaration name: (identifier) @function.definition)
(injection_case stream: (identifier) @variable)

(identifier) @variable
