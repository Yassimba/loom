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
    const open = body.match(OPENER);
    const indent = open?.[1] ?? "";
    const nested = inList && indent.length > 0;
    // Outside a list four columns of indent make an indented code block, and
    // a backtick fence's info string holds no backtick.
    const isFence =
      open !== null &&
      (nested || indent.replaceAll("\t", "    ").length <= 3) &&
      !(open[2]?.startsWith("`") && open[3]?.includes("`"));

    if (!isFence) {
      if (body.trim() === "") afterBlank = true;
      else {
        if (LIST_ITEM.test(body)) inList = true;
        else if (afterBlank && !/^[ \t]/.test(body)) inList = false;
        afterBlank = false;
      }
      text += line;
      i++;
      continue;
    }

    const fence = open[2] as string;
    const closer = new RegExp(
      `^${nested ? "[ \\t]*" : " {0,3}"}${fence[0]}{${fence.length},}[ \\t]*\\r?\\n?$`,
    );
    let end = i + 1;
    while (end < lines.length && !closer.test(lines[end] as string)) end++;
    const closed = end < lines.length;
    const raw = lines.slice(i, closed ? end + 1 : end).join("");
    const contentLines = lines.slice(i + 1, end);
    i = closed ? end + 1 : end;
    if (!nested) inList = false;
    afterBlank = false;

    if (open[3]?.trim().split(/\s+/, 1)[0]?.toLowerCase() !== "mermaid") {
      text += raw;
      continue;
    }
    const ownIndent = new RegExp(`^[ \\t]{0,${indent.length}}`);
    const content = contentLines.map((contentLine) => contentLine.replace(ownIndent, "")).join("");
    if (text) segments.push({ raw: text });
    text = "";
    segments.push({
      raw,
      mermaid: { source: closed ? content.replace(/\r?\n$/, "") : content, indent, closed, nested },
    });
  }
  if (text) segments.push({ raw: text });
  return segments;
}
