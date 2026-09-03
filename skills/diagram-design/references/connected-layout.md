# Connected layout

Load this reference for diagrams whose ordinary nodes are joined by arrows or lines. Type-specific primitives such as Sankey bands, fishbone bones, and loop arcs follow their type reference instead.

## Roles

The active profile supplies `paper`, `paper-2`, `ink`, `muted`, `soft`, `rule`, `rule-solid`, `accent`, `accent-tint`, and `link`. Use `link` for HTTP, API, and external paths.

| Role              | Treatment                               |
| ----------------- | --------------------------------------- |
| focal             | accent tint + accent stroke             |
| backend/step      | paper + ink stroke                      |
| store/state       | faint ink fill + muted stroke           |
| external          | faint ink fill + translucent ink stroke |
| input/user        | muted tint + soft stroke                |
| optional/async    | faint fill + dashed translucent stroke  |
| security/boundary | accent tint + dashed accent stroke      |

A standard node is an opaque paper mask, `rx=6` box, rectangular `rx=2` type tag, human-readable name, and optional technical sublabel. Human names use the profile's sans face; technical strings use mono.

## Connectors

1. Off-axis routes use rounded orthogonal elbows (`r=8`; `r=6` when tight). Straight lines share an x or y coordinate.
2. Labels use an opaque paper mask and sit 6–10px clear of the stroke; vertical labels sit beside it.
3. Paths remain distinct. Offset parallel routes by at least 12px; use a bridge at unavoidable crossings.
4. Multiple paths on one node edge use attach points at least 12px apart.
5. Routes go around non-endpoint nodes. A necessary transit behind one is dashed and labels its visible end.
6. Label masks remain clear of nodes painted after them.

Paint in this order: background → zones → connectors and labels → nodes → legend. Define default, accent, and link arrow markers for directed connectors.

## Budget

Default budget: 9 nodes, 12 connectors, 2 accent elements. Above budget, split overview and detail instead of shrinking text. The selected type reference may override this budget.

Use a 4px grid for authored coordinates, dimensions, padding, gaps, and font sizes. Allowed radii are 4, 6, and 8px. Stroke widths and data-derived coordinates are exempt.
