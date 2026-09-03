# Ridgeline


**Best for:** one **distribution** per series, stacked at a fixed pitch with deliberate overlap, when the shape of each distribution is the story and the series are directly comparable — request latency per service, build duration per pipeline, session length per cohort. A line chart plots one value per x; a ridgeline plots a whole distribution per row, and the reading is the silhouette: where the mass sits, how tight it is, whether there is a second peak. A p50/p99 pair cannot show a bimodal service, and a box plot flattens it to a rectangle.

Not for: a single distribution (that is a histogram — one ridge is a ridgeline with the comparison removed); series measured in different units or on different x-ranges, which cannot share the one x-scale the type requires; time series, where each row is one value per moment rather than one distribution; or distributions so similar that the ridges are five copies of one shape, which a table of percentiles says shorter.

#### Layout conventions

- **One baseline per ridge at a fixed pitch**, inside the `0 0 1000 500` viewBox the other Line variants use. The shipped example runs five baselines at `y` 152/208/264/320/376, a 56px pitch.
- **The x-run spans `x` 320 → 680**, sampled at 13 bins 30px apart. The gutters are symmetric about it: names right-aligned ending at `x=304`, ranges starting at `x=696`, both 16px clear of the plot.
- **Name left of the baseline, range right of it**, each on its ridge's own row (`y = baseline + 3.5`). The range is the span of the ridge's nonzero mass in x-axis units, which is the number a silhouette cannot give you.
- **Bin ticks** in Geist Mono 9px at `y=400`, centred on bin positions and tracked `0.14em`, with the x-axis caption at `y=424`. The rotated amplitude caption sits at `x=24`.
- **Ridge count 3–12, bins 8–40.** Below three ridges there is no family of shapes to compare and two distributions are a pair of small multiples; past twelve the stack is taller than a reader can hold one silhouette in mind across. Below eight bins the outline is a histogram wearing a curve's clothes; past forty the bins are narrower than the noise in them.
- **Straight segments between bins, closed with `Z`.** Not splines: the source line states the drawing is unsmoothed, and a curve through binned counts puts extrema between bins that the sample never measured. The draft this variant came from permits Catmull-Rom with vertices on true values; the shipped grammar does not use it, because unsmoothed bins need no footnote about what the curve is allowed to invent.
- **No gridlines.** The baselines are the rules, and every ridge prints its own range.

#### Colour

Use one accent on the editorially focal ridge (stroke `accent`, fill the accent at `0.16`), an `ink` opacity ramp for the rest (`0.80 → 0.62`, floor `0.53`) ordered top row to bottom, and focus carried by stroke weight — 2.4px against 1.2px — rather than tone. Labels stay `ink` (names) and `muted` (ranges) on every ridge including the focal one.

- **Fill `ink` at `0.12` on every non-focal ridge**, low enough that two overlapping ridges read as depth rather than as a third tone. This is the one place the type needs a fill at all: the outline alone does not say which side of the curve is mass.
- **Legend wording stays skin-neutral** — "strongest tone", never "darkest". The ramp is ink-at-opacity, so its top is the darkest line on light paper and the lightest on dark, and a legend that says "darker" ships false in one of the two skins while rendering perfectly in both.
- **The accent marks the ridge the story is about, not the worst performer.** In the shipped example it marks the service whose *shape* is unusual, not the slowest one.

#### Honest-data rule

**One amplitude on every ridge, stated in the source line.** That is the entire claim of the type: the ridges are stacked so their silhouettes can be compared, and per-ridge normalisation destroys exactly that while rendering beautifully — a rare, flat distribution given its own scale wears the same shape as a tight one.

- **The baseline never lies.** Each ridge declares the row it rises from, and that row must sit on the stack's fixed pitch with its rule drawn across the full bin run. A baseline nudged up to give one ridge headroom is the same falsification as a private amplitude, told with different arithmetic — and it is the harder one to see, because the silhouette above it is untouched.
- **Every ridge shares one x-scale.** A peak at one x means one latency on every row, or the column comparison the type exists for is fiction.
- **State the overlap, and keep it overlap.** Ridges are meant to intrude on the row above — that is how the stack reads as depth. A peak that reaches the row *two* above is occluded by the peaks it passes, and the fix is more pitch, never a smaller amplitude on one ridge. The source line states the amplitude so the reader can convert a height back into a number.
- **A ridge starts and ends on its own baseline.** The first and last bins are zero, so the outline closes along the baseline. A distribution clipped mid-mass draws a cliff, and a cliff reads as data.
- **No smoothing beyond what the footnote states.** The shipped grammar smooths nothing at all. If a figure ever does smooth, the footnote names the method and the vertices stay on true values — a spline that invents a second peak is indistinguishable, on the page, from a service that has one.
- **Height is share, not volume.** Each ridge is normalised to its own series' count before the shared amplitude is applied, so the tallest ridge is the most concentrated, not the busiest. Say so in the source line; a reader who assumes height is traffic reads the figure exactly backwards.

#### Declaring the values

Bind the outline to its bins and baseline, and bind every visible string to what it describes.

```svg
<line data-ridge="checkout-api" data-role="baseline" x1="320" y1="320" x2="680" y2="320" stroke="rgba(45,49,66,0.25)" stroke-width="1"/>
<path data-ridge="checkout-api" data-baseline="320" data-bins="0,1,6,17,21,14,8,6,7,9,7,4,0" d="M320,320 L350,317.6 L380,305.6 L410,279.2 L440,269.6 L470,286.4 L500,300.8 L530,305.6 L560,303.2 L590,298.4 L620,303.2 L650,310.4 L680,320 Z" fill="rgba(235,108,54,0.16)" stroke="#eb6c36" stroke-width="2.4" stroke-linejoin="round"/>
<text data-ridge="checkout-api" data-role="name" x="304" y="323.5" fill="#2d3142" font-size="11" font-weight="600" font-family="'Geist', sans-serif" text-anchor="end">checkout-api</text>
<text data-ridge="checkout-api" data-role="range" x="696" y="323.5" fill="#4f5d75" font-size="9" font-family="'Geist Mono', monospace">40–440 ms</text>
<text data-tick="2" data-bin="240" x="500" y="400" fill="#4f5d75" font-size="9" font-family="'Geist Mono', monospace" letter-spacing="0.14em" text-anchor="middle">240</text>
```

`data-bins` states the values behind the path. `data-baseline` makes a moved row detectable instead of asking a reviewer to infer zero from the drawing. The printed range agrees with the first and last nonzero bin on the shared tick scale.

**Keep verified geometry free of `transform`**: a transform on an outline, baseline, bound label, ancestor `<g>`, or CSS rule moves the rendered mark away from its declared coordinates. The rotated amplitude caption is fine — it is neither verified geometry nor a bound label.

#### Anti-patterns

- Per-ridge normalisation, or any second amplitude anywhere in the figure — the type's one unforgivable error.
- A baseline moved off the pitch to buy one ridge headroom, or a baseline rule drawn somewhere other than the row its ridge declares.
- Ridges sampled on different x-ranges, or an x-scale that is not printed at all.
- Smoothing that puts a peak between two bins, and any spline in a figure whose footnote says the drawing is unsmoothed.
- A ridge clipped mid-mass so the outline closes as a cliff.
- Pitch so tight that peaks hide behind peaks — increase the pitch, never shrink one ridge.
- One hue per ridge instead of the ink ramp plus a single accent.
- A legend that says "darker is faster", which is false on one of the two skins.
- Fewer than 3 ridges (that is a histogram or a pair of small multiples) or more than 12.
- Reading traffic off ridge height, or shipping a figure whose source line lets a reader do so.

## Examples

- `assets/example-ridgeline.html` — minimal light
- `assets/example-ridgeline-dark.html` — minimal dark
- `assets/example-ridgeline-full.html` — full editorial
