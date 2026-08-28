---
version: alpha
name: Signal Rail
description: "A dark technical deck system for platform, product-suite, and engineering decks. A flat near-black navy canvas, one accent colour derived from the deck's own subject, a left rail that tracks which topic a slide belongs to, and a hairline that runs from the slide's eyebrow out to the top-right corner. Ten fixed layouts — nothing is composed freehand. Type is a single variable sans for display and body plus a wide-tracked all-caps label face; the palette carries the meaning, so backgrounds stay flat and decoration stays to hairlines."

colors:
  bg: "#0D1323"
  bg-raised: "#131A2E"
  bg-sunken: "#0A0F1C"
  fg-1: "#F2F4FA"
  fg-2: "#B4BAC9"
  fg-3: "#6E7588"
  fg-4: "#3C4256"
  line-strong: "rgba(242, 244, 250, 0.18)"
  line: "rgba(242, 244, 250, 0.10)"
  line-soft: "rgba(242, 244, 250, 0.06)"
  accent: "derived from the deck subject — see Accent"
  accent-shade: "color-mix(in oklab, var(--accent) 30%, transparent)"
  accent-dim: "color-mix(in oklab, var(--accent) 35%, transparent)"
  accent-ink: "#0D1323"

typography:
  display: { fontFamily: "var(--font-display)", fontSize: "80px", fontWeight: 700, lineHeight: 1.02, letterSpacing: "-0.01em", color: "var(--accent)" }
  slide-h1: { fontFamily: "var(--font-display)", fontSize: "52px", fontWeight: 500, lineHeight: 1.04, maxWidth: "1480px", color: "var(--accent)" }
  h2: { fontFamily: "var(--font-display)", fontSize: "40px", fontWeight: 600, lineHeight: 1.15, color: "var(--fg-1)" }
  h3: { fontFamily: "var(--font-display)", fontSize: "28px", fontWeight: 600, lineHeight: 1.2, color: "var(--fg-1)" }
  eyebrow: { fontFamily: "var(--font-label)", fontSize: "28px", fontWeight: 200, letterSpacing: "0.18em", textTransform: "uppercase", lineHeight: 1, color: "var(--accent)" }
  body: { fontFamily: "var(--font-body)", fontSize: "22px", fontWeight: 300, lineHeight: 1.5, color: "var(--fg-2)" }
  small: { fontSize: "18px", color: "var(--fg-3)" }
  mono: { fontFamily: "var(--font-mono)", fontSize: "18px", color: "var(--fg-2)" }
---

# Signal Rail

A deck built in Signal Rail is a fixed 1920×1080 stage, a flat canvas, and ten layouts. The design work is choosing the accent and the words; the geometry is already decided, which is what keeps thirty slides looking like one deck.

## Accent — derived from the subject

Signal Rail ships neutrals, not a palette. Pick the accent from what the deck is about, and commit to it: it colours every eyebrow, every heading, the rail, the corner hairline, and nothing else competes with it.

- One subject, one accent. Data and flow read blue; energy and throughput read amber; safety, risk, and failure read red; growth and generation read green; craft and design read violet.
- A deck spanning two to five topics (products, teams, phases) gets one accent each, assigned in the same order everywhere. Each slide declares its own with `data-topic="<slug>"`, which flips `--accent` and `--accent-shade` for the whole slide.
- Saturate for a dark canvas: an accent needs to hold at 28px eyebrow weight 200 against `#0D1323`. Mid-tone and bright reads; anything darker than the raised background disappears.
- `--accent-shade` is the same hue at 30%, and it is the only thing hairlines, inactive rail marks, and ambient chrome are ever painted in.

Backgrounds stay flat. The canvas is one colour — no gradients, no mesh, no glow. Depth comes from `--bg-raised` cards and hairlines.

## Type — two families

One variable sans carries display and body across a 100–1000 weight range; one label face carries all-caps eyebrows, page numbers, and numerals. Pick both from the subject and vary them between decks — a technical system for infrastructure, a warmer grotesque for people-facing material. Pair a variable sans (Hubot Sans, Geist, Satoshi, General Sans) with a geometric label face (Outfit, Space Grotesk, Chivo) and a system mono stack. The label face never runs body copy; the display face never runs an eyebrow.

## Stage and chrome

- Stage: 1920×1080, scaled whole to the viewport, per the skill's fixed-stage rules.
- `.slide` is `position: absolute; inset: 0`, background `--bg`, `overflow: hidden`.
- `.slide-content` sits at `left: 240px; top: 82px; right: 120px; bottom: 96px`, a column with `gap: 56px`. Eyebrow to `h1` gap is 36px.
- **Rail** — `left: 80px; top: 82px; bottom: 60px; width: 100px`. Marks stack in a 44px column with an 84px gap; a 1px `--accent-shade` hairline runs at `left: 72px` from the top of the stack to the bottom of the page number, and the two-digit page number closes it. See *The rail* below — it is the deck's agenda, not decoration.
- **Corner** — a single hairline SVG per chrome slide: `M {m} 96 H 1850 Q 1880 96 1895 122 L 2025 347`, stroked 2px in `--accent-shade` with `vector-effect="non-scaling-stroke"`, in a `viewBox="0 0 1920 1080"` with `preserveAspectRatio="none"`. `{m}` is the right edge of the slide's eyebrow plus 60px, computed on load:

```js
document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll(".slide-corner").forEach((svg) => {
    const slide = svg.closest(".slide");
    const label = slide?.querySelector(".s-label, .pt-eyebrow-top, .dv-section-header");
    if (!label) return;
    const scale = 1920 / slide.getBoundingClientRect().width;
    const m = (label.getBoundingClientRect().right - slide.getBoundingClientRect().left) * scale + 60;
    svg.querySelector("path")?.setAttribute("d", `M ${m} 96 H 1850 Q 1880 96 1895 122 L 2025 347`);
  });
});
```

The title slide carries neither rail nor corner. Every other slide carries both.

## The rail

The rail is the deck's agenda, on screen the whole way through. One mark per topic, in the order the deck covers them, so the audience always sees which topic they are in, what has been covered, and what is still coming. It is why a thirty-slide deck stays navigable without a table-of-contents slide.

**Choosing marks.** One icon per topic, each a concrete noun the audience can name — a turbine for throughput, a shield for governance, a graph for reporting. Abstract shapes read as decoration and teach nothing. Draw them as one family: identical optical size on a 24-unit grid, one stroke weight, `fill="currentColor"` and no colour of their own, so `--accent` and `--accent-shade` drive them. Two sources work — a hidden `<symbol>` sprite at the top of `<body>` for bespoke or product marks, or Material Symbols Outlined via its single stylesheet link when the topics are generic. Pick one source per deck; a mixed rail looks assembled.

**States.** Three, and they are what makes the rail readable at a glance:

```css
.rail-mark           { color: var(--accent-shade); }  /* upcoming */
.rail-mark.is-past   { color: var(--fg-4); }          /* covered */
.rail-mark.is-active { color: var(--accent); transform: scale(1.2); transform-origin: left center; }
```

Covered topics recede to the neutral hairline colour, the current one is bright and 1.2×, upcoming ones hold the accent shade — so the eye reads direction down the rail. `data-rail-mode="all"` on a section lights every mark at once, for the slides that speak for the whole deck: the overview, the diagram, the roadmap.

**Scale.** Three to six topics. Past six the marks shrink below recognition and the agenda stops being one glance — group topics until they fit. A single-topic deck drops the icons and runs two-digit numerals in the same column instead.

**Handover.** A `slide--divider` sits between topics: it poses the question the next topic answers and carries the incoming topic's `data-topic`, so the accent turns over and the rail advances on the same slide. That pairing is what makes "what comes next" explicit rather than implied.

**Accessibility.** Each `<symbol>` carries a `<title>` naming its topic, and the active mark gets `aria-current="step"`. The rail sits outside the slide's reveal animation — it never animates in; only the active mark cross-fades when the topic changes.

## Layouts

Ten, and a slide that fits none of them is two slides. Every one lives inside `.slide-content` unless noted.

| Layout | Class | Use |
| --- | --- | --- |
| Title | `slide--title` | Deck opener: hero mark, deck name, rule, author and date |
| Topic title | `slide--topic-title` | Opens a topic: eyebrow, hero mark, name |
| Comparison | `slide--comparison` | Two panels with a `vs` divider |
| Stat | `slide--stat` | One large numeral with a unit and a line of context |
| Cards | `slide--cards` | Three to five cards, each a mark, a name, one line |
| Steps | `slide--steps` | Up to four numbered steps with arrows between |
| Split | `slide--split` | Prose column beside a figure |
| Divider | `slide--divider` | Bridge slide: a question that hands to the next topic |
| Diagram | `slide--diagram` | A `/diagram-design` SVG in a framed well |
| Roadmap | `slide--roadmap` | Dated rows with `data-status="done \| current \| future"` |

### Chrome, on every slide but the title

```html
<div class="slide-rail">
  <div class="rail-marks">
    <svg class="rail-mark is-past"><use href="#mark-intro"></use></svg>
    <svg class="rail-mark is-active" aria-current="step"><use href="#mark-alpha"></use></svg>
    <svg class="rail-mark"><use href="#mark-beta"></use></svg>
  </div>
  <div class="rail-page">02</div>
</div>
<svg class="slide-corner" viewBox="0 0 1920 1080" preserveAspectRatio="none" fill="none" aria-hidden="true"><path d="M 640 96 H 1850 Q 1880 96 1895 122 L 2025 347" stroke="var(--accent-shade)" stroke-width="2" vector-effect="non-scaling-stroke"></path></svg>
```

### Layout bodies

```html
<!-- slide--title (no chrome) -->
<div class="title-mark"><svg class="title-mark-img"><use href="#mark-suite"></use></svg></div>
<div class="title-content">
  <h1 class="title-name">Deck title</h1>
  <div class="title-rule"></div>
  <div class="title-meta">Author · <span data-today></span></div>
</div>

<!-- slide--topic-title -->
<div class="pt-eyebrow-top">One-line description</div>
<svg class="pt-hero-mark"><use href="#mark-alpha"></use></svg>
<div class="pt-content"><div class="pt-name">Topic name</div></div>

<!-- slide--comparison -->
<div class="s-label">Section</div>
<h1 class="s-h1">Heading</h1>
<div class="cmp-grid">
  <div class="cmp-panel">
    <div class="cmp-label">Left label</div>
    <div class="cmp-stat">Left</div>
    <p class="cmp-body">One sentence.</p>
  </div>
  <div class="cmp-vs" aria-hidden="true"><span>vs</span></div>
  <div class="cmp-panel">…</div>
</div>

<!-- slide--stat -->
<div class="stat-row">
  <div class="stat-big">5<span class="stat-unit">tools</span></div>
  <div class="stat-meta"><div class="stat-delta">ONE SYSTEM</div><p>One sentence.</p></div>
</div>

<!-- slide--cards -->
<div class="card-grid card-grid--left-line">
  <div class="t-card" data-topic="alpha">
    <svg class="t-card-mark"><use href="#mark-alpha"></use></svg>
    <div class="t-card-name">Name</div>
    <p class="t-card-body">One line.</p>
  </div>
</div>

<!-- slide--steps -->
<div class="step-flow">
  <div class="step"><div class="step-num">01</div><div class="step-title">Define</div><div class="step-body">One line.</div></div>
  <div class="step-arrow" aria-hidden="true"><svg viewBox="0 0 56 12" fill="none"><path d="M0 6 H48 M42 1 L50 6 L42 11" stroke="currentColor" stroke-width="2" vector-effect="non-scaling-stroke"></path></svg></div>
</div>

<!-- slide--split -->
<div class="split-body">
  <div class="split-prose"><p>First point.</p><p>Second point.</p></div>
  <div class="split-figure"><img src="assets/figure.webp" alt=""></div>
</div>

<!-- slide--divider (outside .slide-content) -->
<div class="s-label dv-section-header">Next</div>
<div class="dv-title">A question that hands off to the next topic.</div>
<div class="dv-rule"></div>

<!-- slide--diagram -->
<div class="diagram-frame"><!-- inline <svg> from /diagram-design --></div>

<!-- slide--roadmap -->
<div class="rm-list">
  <div class="rm-row" data-status="done">
    <div class="rm-date">Q1 26</div>
    <svg class="rm-dot" viewBox="0 0 24 24" aria-hidden="true"><path d="M12 2 21 7v10l-9 5-9-5V7z"></path></svg>
    <div class="rm-row-text"><div class="rm-label">Milestone</div><div class="rm-body"><p>One sentence.</p></div></div>
  </div>
</div>
```

`rm-dot` is filled at `--accent` for `done`, stroked at `--accent` for `current`, stroked at `--fg-4` for `future`.

## Motion

Restrained and technical: `cubic-bezier(0.2, 0.7, 0.1, 1)` at 120 / 220 / 360ms. Slide entry staggers the eyebrow, the heading, then the body block 60ms apart. Rail marks cross-fade when the active topic changes. Nothing loops, nothing floats.

## Content shape

One idea per slide, roughly 50 words, every slide carrying a visual element — the layouts make that structural. Cards run three to five, steps up to four, comparison always two panels. Overflowing a layout means splitting the slide, never shrinking the type.
