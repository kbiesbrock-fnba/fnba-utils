/** Heuristic: does this text look like markdown worth rendering? Conservative
 *  enough not to fire on plain prose. Used by the palette soft command and
 *  (later) the clipboard manager. */
export function looksLikeMarkdown(text: string): boolean {
  const t = text.trim();
  if (t.length < 3) return false;
  const patterns: RegExp[] = [
    /^#{1,6}\s+\S/m,            // ATX heading
    /^\s*```/m,                 // fenced code
    /^\s*~~~/m,                 // fenced code (tilde)
    /^\s*[-*+]\s+\S/m,          // bullet list
    /^\s*\d+\.\s+\S/m,          // ordered list
    /^\s*>\s+\S/m,              // blockquote
    /^\s*\|.*\|.*$/m,           // table row
    /\[[^\]]+\]\([^)]+\)/,      // link
    /\*\*[^*]+\*\*/,            // bold
    /`[^`]+`/,                  // inline code
    /^[-*_]{3,}\s*$/m,          // thematic break
  ];
  return patterns.some((re) => re.test(t));
}
