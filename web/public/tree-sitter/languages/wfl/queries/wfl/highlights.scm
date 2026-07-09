; WFL syntax highlighting

(identifier) @variable

"use" @keyword.import

[
  "pattern"
  "rule"
  "test"
  "scenario"
] @keyword

[
  "if"
  "then"
  "else"
] @keyword.control

[
  "meta"
  "events"
  "match"
  "on"
  "each"
  "where"
  "event"
  "close"
  "and"
  "join"
  "entity"
  "yield"
  "score"
  "key"
  "conv"
  "limits"
  "for"
  "input"
  "expect"
  "options"
  "traffic"
  "stream"
  "gen"
  "injection"
  "near_miss"
  "miss"
  "precision"
  "recall"
  "fpr"
  "latency_p95"
  "seq"
  "use"
  "not"
  "row"
  "tick"
  "hits"
  "hit"
  "origin"
  "fixed"
  "within"
] @keyword

[
  "in"
  "not"
] @keyword.operator

[
  "snapshot"
  "asof"
  "anti"
  "session"
] @keyword.modifier

(boolean) @constant.builtin
(comparison_operator) @operator

[
  "+"
  "-"
  "*"
  "/"
  "%"
  "&&"
  "||"
] @operator

"|" @operator
"|>" @keyword.operator
"->" @keyword.operator

[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "<" ">" ] @punctuation.bracket
[ "," ";" ":" ] @punctuation.delimiter
"." @punctuation.delimiter
"@" @punctuation.special

(comment) @comment
(string) @string
(number) @number
(duration) @number
(percentage) @number
(rate) @number
(version_tag) @constant
(variable) @variable.special
(derive_reference) @variable.special
(close_reason_ref) @variable.builtin

(rule_declaration name: (identifier) @function.definition)
(pattern_declaration name: (identifier) @function.definition)
(test_block name: (identifier) @function.definition)
(scenario_declaration name: (identifier) @function.definition)
(test_block rule: (identifier) @function)
(pattern_invocation pattern: (identifier) @function)

(event_declaration
  alias: (identifier) @variable
  window: (identifier) @type)

(traffic_stream stream: (identifier) @type)
(injection_case rule: (identifier) @function)
(injection_case stream: (identifier) @type)
(seq_block entity: (identifier) @variable)
(scenario_expect_statement rule: (identifier) @function)

(each_clause alias: (identifier) @variable)
(join_clause window: (identifier) @type)
(yield_target target: (identifier) @type)
(entity_clause type: (identifier) @type)
(entity_clause type: (string) @type)

(transform) @function.builtin
(measure) @function.builtin

(function_call
  function: (identifier) @function.call)

(function_call
  object: (identifier) @type
  method: (identifier) @function.method)

(function_call function: (identifier) @function.builtin (#eq? @function.builtin "count"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "sum"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "avg"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "min"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "max"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "distinct"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "fmt"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "baseline"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "has"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "hit"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "contains"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "regex_match"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "startswith"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "endswith"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "len"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "lower"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "upper"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "time_diff"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "time_bucket"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "coalesce"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "mvcount"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "collect_set"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "collect_list"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "first"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "last"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "stddev"))
(function_call function: (identifier) @function.builtin (#eq? @function.builtin "percentile"))

(function_call
  object: (identifier) @type
  method: (identifier) @function.builtin
  (#eq? @function.builtin "has"))

(field_reference
  object: (identifier) @variable
  field: (identifier) @property)

(named_argument name: (yield_field (identifier) @property))
(named_argument name: (yield_field (quoted_ident) @property))
(meta_entry key: (identifier) @property)
(key_item logical: (identifier) @property)
(option_entry key: (identifier) @property)
(option_entry value: (identifier) @constant)
(limit_item value: (identifier) @constant)
(field_assignment field: (identifier) @property)
(field_assignment field: (string) @property)
(attribute key: (identifier) @property)
(field_predicate field: (identifier) @property)

[
  "max_memory"
  "max_instances"
  "max_throttle"
  "on_exceed"
 ] @property

[
  "sort"
  "top"
  "dedup"
  "where"
] @function.builtin
