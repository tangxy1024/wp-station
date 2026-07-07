//! WPL 文本格式化逻辑。

use super::tree::{
    byte_to_char_index, collect_separator_ranges, find_range_at, find_separator_block,
    infer_closing_indent,
};
use tree_sitter::Parser;

/// WPL 代码格式化器：通过轻量扫描与有限结构规则生成稳定输出。
pub struct WplFormatter {
    /// 每级缩进空格数。
    indent: usize,
}

impl Default for WplFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl WplFormatter {
    // 需要保持原样的函数列表（内部不解析管道/逗号/括号）
    const RAW_FUNCS: &[&str] = &[
        "symbol",
        "f_chars_not_has",
        "f_chars_has",
        "kv",
        "f_chars_in",
    ];

    /// 默认 4 空格缩进。
    pub fn new() -> Self {
        Self { indent: 4 }
    }

    /// 自定义缩进宽度（单位：空格）。
    pub fn with_indent(indent: usize) -> Self {
        Self {
            indent: indent.max(1),
        }
    }

    /// 返回格式化结果；调用方可根据错误提示展示给用户。
    pub fn format_content(&self, content: &str) -> Result<String, WplFormatError> {
        self.format_with_error(content)
    }

    /// 兼容旧行为：格式化失败时返回原文。
    pub fn format_content_or_original(&self, content: &str) -> String {
        match self.format_with_error(content) {
            Ok(v) => v,
            Err(_) => content.to_string(),
        }
    }

    /// 对外暴露显式错误版本，便于调试页展示具体问题。
    pub fn format_with_error(&self, content: &str) -> Result<String, WplFormatError> {
        validate_wpl_syntax(content)?;
        self.format(content)
    }

    /// 核心格式化流程：
    /// 1. 统一换行符；
    /// 2. 逐字符扫描输出；
    /// 3. 在收尾阶段合并多余空行。
    fn format(&self, content: &str) -> Result<String, WplFormatError> {
        let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
        let (separator_ranges, ts_ok) = collect_separator_ranges(&normalized);
        let mut out = String::with_capacity(normalized.len() + 64);
        let mut byte_offsets = Vec::new();
        let chars: Vec<char> = normalized
            .char_indices()
            .map(|(idx, ch)| {
                byte_offsets.push(idx);
                ch
            })
            .collect();
        let input_len = normalized.len();

        let mut bracket_stack: Vec<(char, char)> = Vec::new();
        let mut i = 0usize;
        let mut indent = 0usize;
        let mut start_of_line = true;
        let mut line_no = 1usize;

        let bytes = normalized.as_bytes();
        while i < chars.len() {
            let byte_idx = byte_offsets[i];
            if let Some((range_start, range_end)) = find_range_at(&separator_ranges, byte_idx) {
                let slice_start = if byte_idx < range_start {
                    range_start
                } else {
                    byte_idx
                };
                let slice = &normalized[slice_start..range_end];
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push_str(slice);
                line_no = line_no.saturating_add(slice.matches('\n').count());
                start_of_line = slice.ends_with('\n');
                i = byte_to_char_index(&byte_offsets, range_end, input_len);
                continue;
            }

            let c = chars[i];
            let escaped = i > 0 && chars[i - 1] == '\\';

            if c == '"' {
                let next_non_ws = self.next_non_whitespace_pos(&chars, i + 1);
                let comma_follows = next_non_ws.is_some_and(|idx| chars[idx] == ',');
                let has_closing_quote = self.has_closing_quote(&chars, i + 1);
                if comma_follows || !has_closing_quote {
                    self.write_indent_if_needed(start_of_line, indent, &mut out);
                    out.push('"');
                    let new_i = next_non_ws.unwrap_or(i + 1);
                    line_no = line_no
                        .saturating_add(chars[i..new_i].iter().filter(|ch| **ch == '\n').count());
                    i = new_i;
                    start_of_line = false;
                    continue;
                }

                let (literal, consumed) = self.read_string(&chars[i..], line_no)?;
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push_str(&literal);
                line_no = line_no.saturating_add(literal.matches('\n').count());
                i += consumed;
                start_of_line = false;
                continue;
            }

            if c == 'r' && i + 1 < chars.len() && chars[i + 1] == '#' {
                let (literal, consumed) = self.read_raw_string(&chars[i..], line_no)?;
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push_str(&literal);
                line_no = line_no.saturating_add(literal.matches('\n').count());
                i += consumed;
                start_of_line = false;
                continue;
            }

            if c == '#' && i + 1 < chars.len() && chars[i + 1] == '[' {
                let (ann, consumed) = self.read_bracket_block(&chars[i..], '[', ']', line_no)?;
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push_str(
                    &ann.replace('\n', " ")
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                );
                out.push('\n');
                line_no = line_no.saturating_add(ann.matches('\n').count());
                i += consumed;
                start_of_line = true;
                continue;
            }

            if c == '<' {
                let (fmt_block, consumed) =
                    self.read_bracket_block(&chars[i..], '<', '>', line_no)?;
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push_str(&fmt_block);
                line_no = line_no.saturating_add(fmt_block.matches('\n').count());
                i += consumed;
                start_of_line = false;
                continue;
            }

            if c.is_whitespace() {
                if c == '\n' {
                    if !start_of_line {
                        out.push('\n');
                    }
                    start_of_line = true;
                    line_no = line_no.saturating_add(1);
                } else if !start_of_line {
                    out.push(' ');
                }
                i += 1;
                continue;
            }

            if let Some(name_len) = self.starts_with_raw_func(&chars, i, Self::RAW_FUNCS)
                && let Some((block, consumed)) = self.read_raw_func_block(&chars[i..], name_len)
            {
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push_str(&block);
                line_no = line_no.saturating_add(block.matches('\n').count());
                start_of_line = false;
                i += consumed;
                continue;
            }

            if escaped && (c == '(' || c == ')' || c == '{' || c == '}' || c == '|' || c == ',') {
                self.write_indent_if_needed(start_of_line, indent, &mut out);
                out.push(c);
                start_of_line = false;
                i += 1;
                continue;
            }

            match c {
                '{' => {
                    if let Some(end) = find_separator_block(bytes, byte_idx, &separator_ranges) {
                        let slice = &normalized[byte_idx..end];
                        self.write_indent_if_needed(start_of_line, indent, &mut out);
                        out.push_str(slice);
                        line_no = line_no.saturating_add(slice.matches('\n').count());
                        start_of_line = slice.ends_with('\n');
                        i = byte_to_char_index(&byte_offsets, end, input_len);
                        continue;
                    }
                    bracket_stack.push(('{', '}'));
                    self.write_indent_if_needed(start_of_line, indent, &mut out);
                    out.push('{');
                    out.push('\n');
                    indent += 1;
                    start_of_line = true;
                    i += 1;
                }
                '}' => {
                    if let Some((_, expected)) = bracket_stack.pop() {
                        if expected != '}' {
                            return Err(WplFormatError::MismatchedBracket {
                                expected,
                                found: '}',
                                line: line_no,
                            });
                        }
                    } else if ts_ok {
                        let inferred = infer_closing_indent(&out, self.indent);
                        indent = inferred;
                        if !start_of_line {
                            out.push('\n');
                        }
                        self.write_indent_if_needed(true, indent, &mut out);
                        out.push('}');
                        out.push('\n');
                        start_of_line = true;
                        i += 1;
                        continue;
                    } else {
                        return Err(WplFormatError::UnexpectedClosing {
                            close: '}',
                            line: line_no,
                        });
                    }

                    indent = indent.saturating_sub(1);
                    if !start_of_line {
                        out.push('\n');
                    }
                    self.write_indent_if_needed(true, indent, &mut out);
                    out.push('}');
                    out.push('\n');
                    start_of_line = true;
                    i += 1;
                }
                '(' => {
                    if let Some((inner, consumed)) = self.peek_block(&chars[i..], '(', ')')
                        && !inner.contains(',')
                        && !inner.contains('|')
                    {
                        self.write_indent_if_needed(start_of_line, indent, &mut out);
                        out.push('(');
                        out.push_str(inner.trim());
                        out.push(')');
                        line_no = line_no.saturating_add(
                            chars[i..i + consumed]
                                .iter()
                                .filter(|ch| **ch == '\n')
                                .count(),
                        );
                        start_of_line = false;
                        i += consumed;
                        continue;
                    }
                    bracket_stack.push(('(', ')'));
                    self.write_indent_if_needed(start_of_line, indent, &mut out);
                    out.push('(');
                    out.push('\n');
                    indent += 1;
                    start_of_line = true;
                    i += 1;
                }
                ')' => {
                    if let Some((_, expected)) = bracket_stack.pop() {
                        if expected != ')' {
                            return Err(WplFormatError::MismatchedBracket {
                                expected,
                                found: ')',
                                line: line_no,
                            });
                        }
                    } else if ts_ok {
                        let inferred = infer_closing_indent(&out, self.indent);
                        indent = inferred;
                        if !start_of_line {
                            out.push('\n');
                        }
                        self.write_indent_if_needed(true, indent, &mut out);
                        out.push(')');
                        start_of_line = false;
                        i += 1;
                        continue;
                    } else {
                        return Err(WplFormatError::UnexpectedClosing {
                            close: ')',
                            line: line_no,
                        });
                    }

                    indent = indent.saturating_sub(1);
                    if !start_of_line {
                        out.push('\n');
                    }
                    self.write_indent_if_needed(true, indent, &mut out);
                    out.push(')');
                    start_of_line = false;
                    i += 1;
                }
                ',' => {
                    out.push(',');
                    out.push('\n');
                    start_of_line = true;
                    i += 1;
                }
                '|' => {
                    self.write_indent_if_needed(start_of_line, indent, &mut out);
                    if !start_of_line && !matches!(out.chars().last(), Some(' ' | '\n')) {
                        out.push(' ');
                    }
                    out.push('|');
                    out.push(' ');
                    while i + 1 < chars.len() && chars[i + 1].is_whitespace() {
                        if chars[i + 1] == '\n' {
                            line_no = line_no.saturating_add(1);
                        }
                        i += 1;
                    }
                    start_of_line = false;
                    i += 1;
                }
                _ => {
                    self.write_indent_if_needed(start_of_line, indent, &mut out);
                    out.push(c);
                    start_of_line = false;
                    i += 1;
                }
            }
        }

        if let Some((open, close)) = bracket_stack.pop() {
            return Err(WplFormatError::UnclosedBracket {
                open,
                close,
                line: line_no,
            });
        }

        let mut final_out = String::new();
        let mut last_blank = false;
        for line in out.trim_end().lines() {
            let blank = line.trim().is_empty();
            if blank && last_blank {
                continue;
            }
            last_blank = blank;
            final_out.push_str(line);
            final_out.push('\n');
        }

        while final_out.ends_with("\n\n\n") {
            final_out.pop();
        }

        Ok(final_out)
    }

    /// 仅在行首输出缩进，避免中途插入无意义空格。
    fn write_indent_if_needed(&self, start_of_line: bool, indent: usize, buf: &mut String) {
        if start_of_line {
            for _ in 0..indent {
                buf.push_str(&" ".repeat(self.indent));
            }
        }
    }

    /// 读取普通字符串字面量，识别转义字符与闭合引号。
    fn read_string(
        &self,
        input: &[char],
        line_no: usize,
    ) -> Result<(String, usize), WplFormatError> {
        let mut out = String::new();
        let mut escaped = false;
        for (idx, ch) in input.iter().enumerate() {
            out.push(*ch);
            if escaped {
                escaped = false;
                continue;
            }
            if *ch == '\\' {
                escaped = true;
            } else if *ch == '"' && idx > 0 {
                return Ok((out, idx + 1));
            }
        }
        Err(WplFormatError::UnclosedString { line: line_no })
    }

    /// 读取 Rust 风格 raw 字符串：`r###"..."###`。
    fn read_raw_string(
        &self,
        input: &[char],
        line_no: usize,
    ) -> Result<(String, usize), WplFormatError> {
        let mut out = String::new();
        let mut hash_count = 0usize;
        let mut idx = 0usize;

        if input.get(idx) != Some(&'r') {
            return Err(WplFormatError::InvalidRawStringStart { line: line_no });
        }
        out.push('r');
        idx += 1;

        while idx < input.len() && input[idx] == '#' {
            out.push('#');
            hash_count += 1;
            idx += 1;
        }
        if idx >= input.len() || input[idx] != '"' {
            return Err(WplFormatError::InvalidRawStringStart { line: line_no });
        }
        out.push('"');
        idx += 1;

        while idx < input.len() {
            let ch = input[idx];
            out.push(ch);
            if ch == '"' {
                let mut matched = true;
                for h in 0..hash_count {
                    if idx + 1 + h >= input.len() || input[idx + 1 + h] != '#' {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    for _ in 0..hash_count {
                        out.push('#');
                    }
                    return Ok((out, idx + 1 + hash_count));
                }
            }
            idx += 1;
        }
        Err(WplFormatError::UnclosedRawString { line: line_no })
    }

    /// 读取成对括号块，支持嵌套，常用于注解与格式占位片段。
    fn read_bracket_block(
        &self,
        input: &[char],
        open: char,
        close: char,
        line_no: usize,
    ) -> Result<(String, usize), WplFormatError> {
        let mut out = String::new();
        let mut depth = 0usize;
        for (idx, ch) in input.iter().enumerate() {
            out.push(*ch);
            if *ch == open {
                depth += 1;
            } else if *ch == close {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok((out, idx + 1));
                }
            }
        }
        Err(WplFormatError::UnclosedBracket {
            open,
            close,
            line: line_no,
        })
    }

    /// 预览括号块内容，用于判断是否可以保持单行。
    fn peek_block(&self, input: &[char], open: char, close: char) -> Option<(String, usize)> {
        let mut out = String::new();
        let mut depth = 0usize;
        let mut escaped = false;
        let mut in_str = false;
        for (idx, ch) in input.iter().enumerate() {
            if escaped {
                out.push(*ch);
                escaped = false;
                continue;
            }
            match ch {
                '\\' => {
                    out.push(*ch);
                    escaped = true;
                }
                '"' => {
                    out.push(*ch);
                    in_str = !in_str;
                }
                _ if in_str => out.push(*ch),
                _ if *ch == open => {
                    depth += 1;
                    if depth == 1 {
                        continue;
                    }
                    out.push(*ch);
                }
                _ if *ch == close => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some((out, idx + 1));
                    }
                    out.push(*ch);
                }
                _ => out.push(*ch),
            }
        }
        None
    }

    /// 获取从指定位置开始首个非空白字符的下标。
    fn next_non_whitespace_pos(&self, input: &[char], start: usize) -> Option<usize> {
        input
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, ch)| !ch.is_whitespace())
            .map(|(idx, _)| idx)
    }

    /// 判断后续是否存在未被转义的闭合引号。
    fn has_closing_quote(&self, input: &[char], start: usize) -> bool {
        let mut escaped = false;
        for ch in input.iter().skip(start) {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => return true,
                _ => {}
            }
        }
        false
    }

    /// 检测是否匹配需要原样保留内容的 raw 函数名。
    fn starts_with_raw_func(&self, input: &[char], idx: usize, names: &[&str]) -> Option<usize> {
        for name in names {
            let pat: Vec<char> = name.chars().chain(['(']).collect();
            if idx + pat.len() > input.len() {
                continue;
            }
            if input[idx..idx + pat.len()]
                .iter()
                .zip(pat.iter())
                .all(|(a, b)| a == b)
            {
                return Some(name.len());
            }
        }
        None
    }

    /// 读取 raw 函数块，内部内容按原样保留，不解析逗号、管道和括号。
    fn read_raw_func_block(&self, input: &[char], name_len: usize) -> Option<(String, usize)> {
        let mut out = String::new();
        let mut depth = 0i32;
        let mut in_str = false;
        let mut escaped = false;
        let mut seen_func = false;

        for (idx, ch) in input.iter().enumerate() {
            out.push(*ch);
            if !seen_func && idx + 1 == name_len {
                seen_func = true;
            }
            if escaped {
                escaped = false;
                continue;
            }
            if *ch == '\\' {
                escaped = true;
                continue;
            }
            if *ch == '"' {
                in_str = !in_str;
                continue;
            }
            if in_str {
                continue;
            }
            if *ch == '(' {
                depth += 1;
            } else if *ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Some((out, idx + 1));
                }
            }
        }
        None
    }
}

fn validate_wpl_syntax(content: &str) -> Result<(), WplFormatError> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_wpl::language())
        .map_err(|_| WplFormatError::SyntaxError {
            line: 1,
            message: "加载 tree-sitter-wpl 语法失败".to_string(),
        })?;

    let tree = parser
        .parse(content, None)
        .ok_or_else(|| WplFormatError::SyntaxError {
            line: 1,
            message: "WPL 语法树解析失败".to_string(),
        })?;

    let root = tree.root_node();
    if !root.has_error() {
        return Ok(());
    }

    Err(WplFormatError::SyntaxError {
        line: first_tree_sitter_error_line(root).unwrap_or(1),
        message: "WPL 语法树解析失败".to_string(),
    })
}

fn first_tree_sitter_error_line(node: tree_sitter::Node) -> Option<usize> {
    if node.is_error() || node.is_missing() {
        return Some(node.start_position().row + 1);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(line) = first_tree_sitter_error_line(child) {
            return Some(line);
        }
    }
    None
}

/// WPL 格式化过程中的结构化错误。
#[derive(Debug)]
pub enum WplFormatError {
    /// tree-sitter 语法树解析失败。
    SyntaxError { line: usize, message: String },
    /// 普通字符串缺少闭合引号。
    UnclosedString { line: usize },
    /// raw 字符串未正确起始（缺少 `r` 或 `"`）。
    InvalidRawStringStart { line: usize },
    /// raw 字符串缺少闭合引号或井号。
    UnclosedRawString { line: usize },
    /// 任意成对括号缺少闭合。
    UnclosedBracket {
        open: char,
        close: char,
        line: usize,
    },
    /// 括号类型不匹配。
    MismatchedBracket {
        expected: char,
        found: char,
        line: usize,
    },
    /// 遇到多余的闭合括号。
    UnexpectedClosing { close: char, line: usize },
}

impl std::fmt::Display for WplFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WplFormatError::SyntaxError { line, message } => {
                write!(f, "第 {} 行：{}", line, message)
            }
            WplFormatError::UnclosedString { line } => {
                write!(f, "第 {} 行：字符串字面量未闭合", line)
            }
            WplFormatError::InvalidRawStringStart { line } => {
                write!(f, "第 {} 行：raw 字符串起始格式不正确", line)
            }
            WplFormatError::UnclosedRawString { line } => {
                write!(f, "第 {} 行：raw 字符串未闭合", line)
            }
            WplFormatError::UnclosedBracket { open, close, line } => {
                write!(f, "第 {} 行：括号未闭合：{} ... {}", line, open, close)
            }
            WplFormatError::MismatchedBracket {
                expected,
                found,
                line,
            } => {
                write!(
                    f,
                    "第 {} 行：括号不匹配，期望 {}，但遇到 {}",
                    line, expected, found
                )
            }
            WplFormatError::UnexpectedClosing { close, line } => {
                write!(f, "第 {} 行：多余的闭合括号 {}", line, close)
            }
        }
    }
}

impl std::error::Error for WplFormatError {}
