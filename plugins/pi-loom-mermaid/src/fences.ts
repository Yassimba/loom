/** A run of a markdown document: text passed through as written, or one Mermaid fence. */
export type Segment = {
  raw: string;
  mermaid?: {
    /** The diagram source, the fence's own indentation removed. */
    source: string;
    indent: string;
    closed: boolean;
    /** Inside a list item, where the drawing keeps the item's indentation. */
    nested: boolean;
  };
};

const OPENER = /^([ \t]*)(`{3,}|~{3,})[ \t]*(.*)$/;
const LIST_ITEM = /^ {0,3}(?:[-*+]|\d{1,9}[.)])(?:[ \t]|$)/;

type Fence = NonNullable<Segment["mermaid"]>;
type Opener = { indent: string; fence: string; info: string; nested: boolean };

/**
 * The fence a line opens, if any. Outside a list four columns of indent make
 * an indented code block, and a backtick fence's info string holds no backtick.
 */
function opener(body: string, inList: boolean): Opener | null {
  const open = body.match(OPENER);
  if (open === null) return null;
  const [, indent = "", fence = "", info = ""] = open;
  const nested = inList && indent.length > 0;
  if (!nested && indent.replaceAll("\t", "    ").length > 3) return null;
  if (fence.startsWith("`") && info.includes("`")) return null;
  return { indent, fence, info, nested };
}

/** The line closing the fence opened at `start`, or `lines.length` when none does. */
function closerAt(lines: string[], start: number, { fence, nested }: Opener): number {
  const closer = new RegExp(
    `^${nested ? "[ \\t]*" : " {0,3}"}${fence[0]}{${fence.length},}[ \\t]*\\r?\\n?$`,
  );
  let end = start + 1;
  while (end < lines.length && !closer.test(lines[end] as string)) end++;
  return end;
}

/** A list stays open until an unindented line follows a blank one. */
function isListOpen(body: string, inList: boolean, afterBlank: boolean): boolean {
  if (LIST_ITEM.test(body)) return true;
  return inList && !(afterBlank && !/^[ \t]/.test(body));
}

function mermaidFence(content: string[], open: Opener, closed: boolean): Fence | undefined {
  if (open.info.trim().split(/\s+/, 1)[0]?.toLowerCase() !== "mermaid") return undefined;
  const ownIndent = new RegExp(`^[ \\t]{0,${open.indent.length}}`);
  const source = content.map((line) => line.replace(ownIndent, "")).join("");
  return {
    source: closed ? source.replace(/\r?\n$/, "") : source,
    indent: open.indent,
    closed,
    nested: open.nested,
  };
}

/**
 * Split markdown at its Mermaid fences. Every fence is tracked, so a Mermaid
 * example quoted inside another code block stays text; joining each `raw`
 * gives the document back.
 */
export function scanFences(markdown: string): Segment[] {
  const lines = markdown.match(/[^\n]*\n|[^\n]+/g) ?? [];
  const segments: Segment[] = [];
  let text = "";
  let inList = false;
  let afterBlank = false;

  for (let i = 0; i < lines.length; ) {
    const line = lines[i] as string;
    const body = line.replace(/\r?\n$/, "");
    const open = opener(body, inList);
    if (open === null) {
      const isBlank = body.trim() === "";
      if (!isBlank) inList = isListOpen(body, inList, afterBlank);
      afterBlank = isBlank;
      text += line;
      i++;
      continue;
    }

    const end = closerAt(lines, i, open);
    const closed = end < lines.length;
    const next = closed ? end + 1 : end;
    const raw = lines.slice(i, next).join("");
    const mermaid = mermaidFence(lines.slice(i + 1, end), open, closed);
    i = next;
    inList = inList && open.nested;
    afterBlank = false;

    if (mermaid === undefined) {
      text += raw;
      continue;
    }
    if (text) segments.push({ raw: text });
    text = "";
    segments.push({ raw, mermaid });
  }
  if (text) segments.push({ raw: text });
  return segments;
}
