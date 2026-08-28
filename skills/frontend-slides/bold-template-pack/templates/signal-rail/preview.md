# Signal Rail Preview Card

Use this small file for title-slide previews only. For final deck generation, read the full design doc listed below.

## Files

- Full design doc: `bold-template-pack/templates/signal-rail/design.md`
- Preview card: `bold-template-pack/templates/signal-rail/preview.md`

## Selection Metadata

- Slug: `signal-rail`
- Tagline: Flat near-black canvas, one subject-derived accent, a topic rail that shows where you are and what is next, and a hairline running from the eyebrow to the corner.
- Mood: technical, systematic, platform, engineering, briefing
- Tone: precise, informative, quietly confident, unsold
- Formality: high
- Density: medium
- Scheme: dark
- Best for: Platform and product-suite decks, architecture and engineering briefings, roadmaps, internal strategy reviews, anything covering several topics that must read as one system. The default for technical subject matter.
- Avoid for: Decks that want warmth, playfulness, or photographic drama — the system is hairlines and type on a flat dark canvas, with one accent doing all the work.

## Visual Snapshot

A fixed 1920×1080 stage on a flat `#0D1323` canvas. A left rail carries one icon per topic in agenda order — the current topic bright and enlarged, covered ones dimmed to neutral, upcoming ones in the accent shade — over a hairline, with a two-digit page number at its foot. Each slide opens with a wide-tracked all-caps eyebrow in the accent, a 52px heading beneath it, and a single hairline that leaves the eyebrow and turns into the top-right corner. The chrome repeats on every slide; each body is composed for its own content, and carries a figure that shows the idea.

The accent is not shipped — it comes from the deck's subject (blue for data and flow, amber for energy, red for risk, green for generation), and a multi-topic deck assigns one accent per topic, which the rail follows slide by slide.

## Preview Ingredients

- Palette: bg #0D1323; raised #131A2E; fg #F2F4FA / #B4BAC9 / #6E7588; accent chosen from the subject
- Typography: one variable sans (Hubot Sans, Geist, Satoshi, General Sans) for display and body; one geometric label face (Outfit, Space Grotesk, Chivo) for all-caps eyebrows and numerals
- Signature move: the eyebrow-to-corner hairline, its start recomputed from the label width on load.
- Signature move: the topic rail — one named icon per topic, three states (covered, current, upcoming), so the agenda is always on screen.
- Signature move: flat canvas — no gradients anywhere; depth is hairlines and raised cards only.
