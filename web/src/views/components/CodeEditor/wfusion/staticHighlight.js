import { Decoration, EditorView, ViewPlugin } from '@codemirror/view';

function buildExclusionRanges(text, patterns) {
  const ranges = [];

  for (const pattern of patterns) {
    const regex = new RegExp(pattern.source, pattern.flags.includes('g') ? pattern.flags : `${pattern.flags}g`);
    for (let match = regex.exec(text); match; match = regex.exec(text)) {
      if (!match[0]) {
        if (regex.lastIndex === match.index) {
          regex.lastIndex += 1;
        }
        continue;
      }
      ranges.push({
        from: match.index,
        to: match.index + match[0].length,
      });
    }
  }

  ranges.sort((left, right) => left.from - right.from || left.to - right.to);
  return ranges;
}

function isInsideRanges(ranges, from, to) {
  return ranges.some((range) => from < range.to && to > range.from);
}

function buildDecorations(text, rules, excludedRanges) {
  const ranges = [];

  for (const rule of rules) {
    const regex = new RegExp(rule.pattern.source, rule.pattern.flags.includes('g') ? rule.pattern.flags : `${rule.pattern.flags}g`);
    for (let match = regex.exec(text); match; match = regex.exec(text)) {
      const value = match[0];
      if (!value) {
        if (regex.lastIndex === match.index) {
          regex.lastIndex += 1;
        }
        continue;
      }

      const from = match.index;
      const to = from + value.length;
      if (isInsideRanges(excludedRanges, from, to)) {
        continue;
      }

      ranges.push(
        Decoration.mark({ class: rule.className }).range(from, to),
      );
    }
  }

  return Decoration.set(ranges, true);
}

export function createStaticHighlightExtension(options) {
  return ViewPlugin.fromClass(
    class {
      constructor(view) {
        this.decorations = this.recompute(view);
      }

      update(update) {
        if (update.docChanged) {
          this.decorations = this.recompute(update.view);
        }
      }

      recompute(view) {
        const text = view.state.doc.toString();
        const excludedRanges = buildExclusionRanges(text, options.excludedPatterns || []);
        return buildDecorations(text, options.rules, excludedRanges);
      }
    },
    {
      decorations: (value) => value.decorations,
    },
  );
}
