/** Newest complete prefix first; never expose a half-written label or comment. */
export function* streamingPrefixes(text: string): Generator<string> {
  const ends: number[] = [];
  let depth = 0;
  // Consume quotes (including unfinished ones) and comments as opaque spans.
  const tokens = /%%[^\n]*|"(?:\\[\s\S]?|[^"\\])*(?:"|$)|[[\]()\n;]/g;
  for (const match of text.matchAll(tokens)) {
    const c = match[0];
    if (c === "[" || c === "(") depth++;
    else if (c === "]" || c === ")") depth = Math.max(0, depth - 1);
    else if (depth === 0 && (c === "\n" || c === ";")) ends.push(match.index + 1);
  }
  for (let i = ends.length - 1; i >= 0; i--) yield text.slice(0, ends[i]);
}
