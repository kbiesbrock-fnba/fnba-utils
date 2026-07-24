import MarkdownIt from "markdown-it";

// html:false escapes raw HTML → safe to inject with v-html, no sanitizer needed.
// linkify auto-links bare URLs. Tables + strikethrough are on by default.
const md = new MarkdownIt({ html: false, linkify: true, breaks: false });

// Tag every block-level open token with its 0-based source line. markdown-it's
// default token renderer emits attrs, so headings, paragraphs, lists, list
// items, blockquotes, tables, and hr all carry data-source-line — used by the
// viewer for scroll sync and click-to-locate. CSP-safe (plain data attr).
md.core.ruler.push("source_line", (state) => {
  for (const token of state.tokens) {
    if (token.map && token.type.endsWith("_open")) {
      token.attrSet("data-source-line", String(token.map[0]));
    }
  }
});

export function renderMarkdown(src: string): string {
  return md.render(src ?? "");
}
