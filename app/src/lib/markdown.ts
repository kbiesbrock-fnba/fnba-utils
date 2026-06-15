import MarkdownIt from "markdown-it";

// html:false escapes raw HTML → safe to inject with v-html, no sanitizer needed.
// linkify auto-links bare URLs. Tables + strikethrough are on by default.
const md = new MarkdownIt({ html: false, linkify: true, breaks: false });

export function renderMarkdown(src: string): string {
  return md.render(src ?? "");
}
