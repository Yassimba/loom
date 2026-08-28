# Style Discovery

Read this when the deck overrides the Signal Rail default and the user needs to choose a look. Most people cannot describe a design in words, so the choice is made by looking: build three real title slides and open them.

## The three previews

Build one preview per slot, each a self-contained single-slide HTML file with one animated title slide, saved to `.frontend-slides/slide-previews/` as `style-a.html`, `style-b.html`, `style-c.html`. Open all three for the user.

| Slot         | Source                                                              |
| ------------ | ------------------------------------------------------------------- |
| **Safe**     | One preset from [STYLE_PRESETS.md](STYLE_PRESETS.md)                |
| **Bold**     | One template from `bold-template-pack/selection-index.json`, Signal Rail excluded |
| **Wildcard** | A second bold template, or a design you author for this brief alone |

Signal Rail is the first entry in that index and is the default this flow exists to replace, so it takes no slot here — a brief that rejected a flat dark technical system is not served by being handed it back.

Pick the wildcard's source by which one contrasts hardest with the other two. When the brief has a sharper opportunity than anything in the library, author the wildcard freely — the library is a shortcut, not a boundary. When a named preset or template arrives in the brief, it takes one slot and the other two are built around it.

Read the two indexes and stop there: `STYLE_PRESETS.md`, and `selection-index.json` if it exists. After shortlisting, read only the shortlisted templates' `preview.md` files, at the `preview_md` paths the index gives. Full `design.md` files are read in Phase 3, after the user picks one.

**Match the stakes.** For a conservative or high-stakes deck, make the safe preset especially restrained, choose a calm high-formality template, and make the wildcard authoritative rather than decorative. For an expressive deck, keep the safe preset as the readable fallback, choose one strong template, and make the wildcard adventurous and specific.

**Reading the index.** Match purpose and mood against `mood`, `tone`, `best_for`, `avoid_for`, `formality`, `density`, and `scheme`. Treat `best_for` examples as soft signals — an "investor pitch" template suits a research readout when the mood matches. When no template matches well, spend the slot on a custom design or a second preset rather than forcing a weak match.

## Mood → preset

| Mood                | Presets                                            |
| ------------------- | -------------------------------------------------- |
| Impressed/Confident | Bold Signal, Electric Studio, Dark Botanical       |
| Excited/Energized   | Creative Voltage, Neon Cyber, Split Pastel         |
| Calm/Focused        | Notebook Tabs, Paper & Ink, Swiss Modern           |
| Inspired/Moved      | Dark Botanical, Vintage Editorial, Pastel Geometry |

If the user gave a vibe, use it. If not, infer the mood from the occasion, audience, content, and stakes, and keep the three far enough apart that the user can react to them visually.

## Authoring a custom wildcard

A custom preview must imply a whole design system, not one pretty slide — it has to expand into section, content, quote, comparison, and closing slides. Give it a deliberate thesis: distinctive typography, a committed palette, a recognisable layout logic, and one strong atmospheric or graphic device. Follow the *Design* section of SKILL.md, and build it on the same fixed 1920×1080 stage as every other option.

## Preview authenticity

Every preview is a real first slide of the user's deck. It carries the user's own words — the deck title, a section title, a genuine phrase from their material — plus real deck chrome: date, author, company, page number.

Workflow language stays in the chat message, never on the slide: template and preset names, `Option A/B/C`, "safe" and "bold", file names and paths, and the requirement notes the user gave you ("sharp and provocative", "for internal sharing", "audience: …"). Read the visible text of each preview before you open it, and rewrite anything that names the process instead of the subject.

## The pick

Ask, header "Style": *Which style preview do you prefer?* — Options: `Style A: [Name]` / `Style B: [Name]` / `Style C: [Name]` / `Mix elements`. On "Mix elements", ask which parts of which.

When the user rejects all three, ask what missed — the mood, the palette, the type, or the density — and build one more round of three against that answer. If a second round also misses, offer the Signal Rail default as the way out: it is settled, technical, and its accent and type still come from the deck's own subject.

The picked preview's CSS and layout become the design recipe for Phase 3: its fonts, palette, spacing rhythm, and layout logic carry into every slide of the deck, and any layout the deck needs but the preview lacks is designed from that same system.
