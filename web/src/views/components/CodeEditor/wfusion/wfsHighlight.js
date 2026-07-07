import { createStaticHighlightExtension } from './staticHighlight';

const excludedPatterns = [
  /\/\/.*/g,
  /`[^`]+`/g,
  /"(?:[^"\\]|\\.)*"/g,
];

const rules = [
  { pattern: /\/\/.*/g, className: 'cm-wf-comment' },
  { pattern: /`[^`]+`/g, className: 'cm-wf-string' },
  { pattern: /"(?:[^"\\]|\\.)*"/g, className: 'cm-wf-string' },
  { pattern: /\bwindow\b/g, className: 'cm-wf-keyword' },
  { pattern: /\b(?:stream|time|over|fields)\b/g, className: 'cm-wf-keyword' },
  { pattern: /\b(?:array|chars|digit|float|bool|time|ip|hex)\b/g, className: 'cm-wf-type' },
  { pattern: /\b\d+[smhd]\b/g, className: 'cm-wf-number' },
  { pattern: /\b\d+\b/g, className: 'cm-wf-number' },
  { pattern: /\b[A-Za-z_][A-Za-z0-9_]*\b(?=\s*\{)/g, className: 'cm-wf-function' },
  { pattern: /\b[A-Za-z_][A-Za-z0-9_.]*\b(?=\s*:)/g, className: 'cm-wf-property' },
  { pattern: /\b[A-Za-z_][A-Za-z0-9_]*\b(?=\s*=)/g, className: 'cm-wf-property' },
  { pattern: /[{}[\](),.:]/g, className: 'cm-wf-punctuation' },
  { pattern: /=|\//g, className: 'cm-wf-operator' },
];

export function wfsHighlightExtension() {
  return createStaticHighlightExtension({
    excludedPatterns,
    rules,
  });
}
