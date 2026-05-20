/**
 * djb2 + FNV-1a combined 64-bit hash, base-36 encoded.
 *
 * Used to derive stable, short panel labels from sessionIds and server names.
 * Doubling djb2 with FNV-1a pushes the birthday-collision threshold to 2^32 —
 * out of reach for the small set of sessions/connections we ever label.
 */
export function hashStr(s: string): string {
  let h1 = 0;
  let h2 = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i);
    h1 = ((h1 << 5) - h1 + c) | 0;
    h2 = Math.imul(h2 ^ c, 16777619);
  }
  return (h1 >>> 0).toString(36) + (h2 >>> 0).toString(36);
}
