# Annotation build

Plannotator's annotate session serves exactly one file. Relative image links 404, and its markdown renderer strips inline `<svg>`. Only its raw-HTML surface renders SVG. So the walkthrough ships as two artifacts:

- `walkthrough.md` — the repo artifact. Standard `![caption](diagrams/<file>.svg)` links; GitHub and editors render them.
- `walkthrough.html` — the annotation artifact, built from the `.md`, with every SVG inlined. This is the file `plannotator annotate` opens.

## Build

```bash
python3 scripts/build-html.py ai-docs/explanations/<slug>/walkthrough.md
```

`build-html.py` runs pandoc (gfm → html), replaces each `<img src="…svg">` with the SVG's content inside a `<figure>`, and wraps the result in page chrome that matches `draw.py`'s skin: same paper/ink/muted/coral palette, prose capped at `70ch`, figures full width, themed `::selection`, `:focus-visible`, `scrollbar-color`, `caret-color`. It prints the figure count and exits 1 when no figure was inlined.

## Launch and verify

Load this launch only when the user asks to annotate.

```bash
plannotator annotate walkthrough.html --json
```

Run it in the background with no timeout; it blocks until the reader submits and then prints one JSON record (`decision`, `feedback`). To verify the figures arrived before telling the reader, curl the session's `/api/plan` and count `<svg` in `rawHtml`; `renderAs` must be `html`.

Rebuild the `.html` after any figure or prose change; the two artifacts drift otherwise.
