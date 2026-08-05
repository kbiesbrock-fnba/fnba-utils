// Small path helpers shared across File Viewer bodies (JSON, Markdown).

/** Last path segment (handles both `/` and `\` separators). */
export function baseName(p: string): string {
  return p.split(/[\\/]/).pop() || p;
}
