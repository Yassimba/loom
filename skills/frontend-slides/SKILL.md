---
name: frontend-slides
description: Build a self-contained HTML slide deck that runs in the browser. Use when the user wants a presentation, slides for a talk or a pitch, a PPT/PPTX converted to web, or an existing HTML deck improved.
---

# Frontend Slides

One HTML file, opened in a browser, is the whole deliverable: CSS and JS inline, no npm, no build step, no runtime dependency. Inside it, slides are drawn at a fixed 1920×1080 and the stage scales as one piece to whatever screen it lands on.

Three rules carry most of the quality:

- **The stage is fixed** — content never reflows per device.
- **The chrome repeats** — whatever frames the content (the eyebrow, the heading position, the margins, the page number, and whatever device the style adds) is identical on every slide and never redesigned for one slide's sake. That repetition is what makes thirty slides read as one deck.
- **The body shows the idea** — inside that frame there is no layout catalogue: compose each body from what the slide has to show, as a figure by default and prose only where an idea has no shape.

## The stage

These hold for every slide of every deck:

- A `.deck-viewport` fills the window; a `.deck-stage` inside it is exactly 1920×1080 with `transform-origin: 0 0`; one JS transform scales and centres it. Letterboxing is the correct result on an odd aspect ratio.
- Author every measurement at the 1920×1080 design size — fixed px, at that scale. `clamp()` belongs to UI outside the stage (controls, chrome) and to small standalone previews.
- Switch slides with `.active` / `.visible` driving `visibility`, `opacity`, and `pointer-events`, exactly as `viewport-base.css` defines them. Keep `display` out of slide switching: a later layout rule such as `.slide-content { display: flex }` overrides a `display: none`, and every slide paints at once.
- Negate a CSS function through `calc(-1 * clamp(...))`. Written directly, `-clamp(...)` is silently ignored.
- Honour `prefers-reduced-motion`.

Read [viewport-base.css](viewport-base.css) and paste its full contents into every deck's `<style>` block.

## Density

Ask which one the deck is (Phase 1), then design to that answer:

| Density                          | Best for                                              | Design behaviour                                                                                                                         |
| -------------------------------- | ----------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| **Low / speaker-led**            | Public talks, keynotes, live explanation              | One idea per slide as one figure, large type, generous negative space, more slides rather than fuller ones; a prose body runs 1–3 lines   |
| **High / reading-first**         | Reports, handouts, async review, internal detail docs | Self-contained slides, an annotated figure each, structured grids and tables, 4–8 lines or 4–6 cards, tighter but still intentional spacing |

Mixed signals resolve to the nearer mode rather than a blend: live persuasion is low, async circulation is high.

Whichever mode is in force, a slide fits the stage unscrolled, panels never overlap, and the smallest text holds at 18px. Content that exceeds the mode becomes another slide — the type size stays.

**Vertical composition.** A body shorter than its box leaves a remainder, and where that remainder sits is a decision, not a leftover. Two placements read as authored: the body fills down to the bottom margin, or it centres so the remainder splits evenly above and below. **Underfill** is the third — the whole remainder parked in one band beneath the content — and it reads as a body that failed to load, at 200px as surely as at 600px. `check-deck.py` fails it.

Underfill is a content problem, and re-centring only moves the empty band. Give the slide something: a divider carrying a question and one line is underfilled, so it carries the topic's agenda too; three items with half a stage spare take a fourth, or a figure. Within those two placements, top-align a dense slide — a table, a stepped figure, a slide the reader scans — so the eye starts at a constant height across the deck, and centre a sparse one — a single statement, one number, one figure with a caption.

## Design

Every deck should look authored for its subject. Work the four levers:

- **Typography** — pick faces with character from Fontshare or Google Fonts, and pair a display face with a distinct text face. Vary the pairing between decks; a deck about infrastructure and a deck about a school fair should not share a font stack.
- **Colour** — commit to one dominant colour with a sharp accent, derived from the subject, and drive it through CSS variables. IDE themes and cultural aesthetics are good hunting grounds. Alternate between light and dark grounds across decks.
- **Motion** — spend the budget on one well-orchestrated entrance: staggered reveals with `animation-delay`, CSS-only. See [animation-patterns.md](animation-patterns.md) for the effect-to-feeling map.
- **Ground** — build atmosphere with layered gradients, geometric pattern, or a graphic device that belongs to the subject.

The failure mode is convergence: across generations the model drifts toward the same few fonts (Inter, Space Grotesk), the same purple-on-white gradient, and the same card grid. Before committing a palette or a type pairing, name what makes it specific to *this* subject — if the answer is "it looks nice", pick again.

## Phase 0: Mode

- **New deck** → Phase 1.
- **PPT conversion** → Phase 4.
- **Enhancement of an existing deck** → read the file, then apply the rules below.

**Enhancing an existing deck.** Fitting is the risk: the stage is full long before the file looks full.

1. Count what a slide already carries and check it against the density table before adding anything.
2. Give new content its own slide when the target slide is at capacity. This is the default for images: move the image to a new slide, or cut something first.
3. Split proactively and say so — an overflowing slide becomes two, without waiting to be asked.
4. Verify by screenshot at 1280×720 and at one phone viewport: 16:9 held, no text past its card, no panels overlapping.

## Phase 1: Content discovery

Ask all four at once, through the environment's structured-question UI when it has one, otherwise as one numbered message:

| # | Header   | Question                                | Options                                                                              |
| - | -------- | --------------------------------------- | ------------------------------------------------------------------------------------ |
| 1 | Purpose  | What is this presentation for?          | Pitch deck / Teaching-tutorial / Conference talk / Internal presentation             |
| 2 | Length   | Approximately how many slides?          | Short 5-10 / Medium 10-20 / Long 20+                                                 |
| 3 | Content  | Do you have content ready?              | All content ready / Rough notes / Topic only                                         |
| 4 | Density  | How dense should the deck feel?         | Low density, speaker-led / High density, reading-first                                |

Then ask for the content itself if they have it. Keep the density answer: it sets slide count, type scale, words per slide, and whether slides are cinematic or self-contained.

Inline editing ships by default and is not one of the questions — nobody can judge an editing affordance before seeing a draft. Build a locked, export-only file only when the user asks for one.

### Images

When the user supplies an image folder, before designing anything:

1. **Scan** — list every image file.
2. **Inspect** each one and record what it shows, whether it is usable and why, the concept it carries, and its dominant colours. Where image reading is unavailable, work from filenames and ask the user only where it matters.
3. **Co-design the outline** — the images shape the slide structure alongside the text, from the start: three screenshots suggest three feature slides, a logo takes the title and closing slides. Planning slides and then hunting for places to drop images produces the worse deck.
4. **Confirm** — "Does this slide outline and image selection look right?" / Looks good / Adjust images / Adjust outline.

A usable logo goes into every Phase 2 preview as base64, so the user sees their own brand in each direction.

Decks with no images are a first-class path: CSS gradients, shapes, and pattern carry the visual weight, alongside the figures from Phase 3.

## Phase 2: Style

**Signal Rail is the default.** Read [bold-template-pack/templates/signal-rail/design.md](bold-template-pack/templates/signal-rail/design.md), derive the accent from the subject (one accent, or one per topic when the deck spans several, which the rail then tracks), pick the two type families for the subject, tell the user the direction in a sentence, and go to Phase 3. The previews are skipped because the chrome is already settled — colour, type, and each slide's figure are the open choices.

One thing overrides the default: **a brief that argues against a flat dark technical system** — warm, consumer, marketing, or celebratory work — or the user asking to see options. Then read [style-discovery.md](style-discovery.md) and run the three-preview flow instead.

## Phase 3: Generate

Read [html-template.md](html-template.md) for the HTML architecture and the JS the deck needs, and [diagrams.md](diagrams.md) before composing the first slide body.

Read the full `design.md` of the one chosen template, and treat it as the recipe: its fonts, palette, decorative vocabulary, spacing rhythm, and component grammar carry into every slide. Fix its chrome on the first slide you build and repeat that exact frame for the rest of the deck. Leave the other templates unread. A custom wildcard's preview is its own recipe, read the same way — the deck expands that system rather than switching to a library style, and designs any missing layout from within it.

Translate the recipe onto the fixed stage: a `design.md` written in viewport-fluid units is giving you proportions, and those become 1920×1080 coordinates in the output, not live reflow rules. Take the system, leave the demo content — copied example slides read as a template someone forgot to fill in.

Every deck ships as one self-contained HTML file with `viewport-base.css` inlined whole, fonts from Fontshare or Google Fonts, and a `/* === SECTION NAME === */` banner over each block of CSS and JS. A deck meant to be presented live carries a `<aside class="notes" hidden>` per slide and the presenter window from [html-template.md](html-template.md).

**Verify with the checker.** A deck is done when this exits 0:

```bash
uv run --with playwright python scripts/check-deck.py <deck.html>
```

It measures every slide after its entrance animation settles, and fails on: content past the stage edge, one painted panel covering another, an underfilled body, text under 18px, contrast under AA, an untagged figure, a diagram form repeated on consecutive slides, and a stage that reflows instead of letterboxing at 720p and phone size. Screenshots land beside the deck.

Read the screenshots too. The checker proves a deck is not broken; only your eyes tell you it is good.

## Phase 4: PPT conversion

1. **Extract** — `python scripts/extract-pptx.py <input.pptx> <output_dir>` (`pip install python-pptx` if missing).
2. **Confirm** — show the user the extracted slide titles, content summaries, and image counts.
3. **Style** — Phase 2.
4. **Generate** — Phase 3, carrying over every string, the images from `assets/`, the slide order, and the speaker notes as HTML comments.

## Phase 5: Deliver

1. **De-slop the copy** — run `/stop-the-slop` over every visible string: headlines, body copy, captions, figure labels, speaker notes. Stance follows the Phase 1 content answer — *Preserve* when the user supplied the words, *Improve* when they came from a topic or rough notes. Slide copy is the shortest and most-read text in the deck, so slop surfaces there first. Done when every string has been through the pass, the rewrites are back in the HTML, and any slide whose copy grew has been re-checked for fit.
2. **Clean up** — delete `.frontend-slides/slide-previews/` if it exists.
3. **Open** — `open [filename].html`.
4. **Summarise** — file location, style name, slide count; navigation (arrows, space, swipe); how to retune it (`:root` variables for colour, the font link for type, `.reveal` for animation); that inline editing is there (hover the top-left corner or press `E`, click any text, `Ctrl+S` to save); presenter mode on `P` when the deck carries speaker notes; and the natural next moves — revisions, direct editing, sharing.
5. **Offer annotation review** — when `plannotator` is installed, ask: *"Would you like me to open the whole deck in Plannotator so you can annotate it?"* On yes, follow [plannotator-review.md](plannotator-review.md). The review surface must show every slide in one scrollable document; the normal one-slide presentation view is incomplete.

## Phase 6: Share

Ask once: *"Would you like to share this presentation? I can deploy it to a live URL (works on any device including phones) or export it as a PDF."* — Deploy / PDF / Both / No thanks.

On a yes, read [sharing.md](sharing.md). On a no, the work is finished.
