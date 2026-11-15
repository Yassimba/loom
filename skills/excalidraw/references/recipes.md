# Excalidraw recipes for the 45 types

One row per `diagram-design` type. The type reference gives the layout grammar; the row says how to build it from Excalidraw elements and the failure to look for in the screenshot. Styles and primitives are in `SKILL.md` § Translation.

## Structure and topology

| Type | Build | Look for |
|---|---|---|
| architecture | nodes in tiers inside zones; bound straight arrows between aligned nodes | diagonal arrows: realign the tiers |
| architecture-delta | two `frame`s, Before and After, with each node at the same position in both; changed nodes accent; a "Changes" free-text list under the frames | a node that moved between frames without a change |
| it-state | one zone per department or phase, as columns; system nodes inside; legacy nodes muted, pain points negative | more than one accent |
| high-level | left-to-right zones (sources, ingestion, storage, serving) inside one cluster `frame` | cross-zone arrows through other zones |
| deployment | nested zones (region > cluster > host), artifact nodes inside; ports as mono free text | a zone label hidden behind a child |
| dp-integration | sources column, core platform zone, consumers column; arrows only into and out of the core | arrows that skip the core |
| dependency | nodes in rows by depth (topological order); arrows point down; cycle edges negative | arrows that point up without being a cycle |
| nested | concentric rectangles, largest first, each with a lower opacity than its parent; labels top-left | labels centred inside the rectangles |
| layers | full-width rectangles of equal width stacked top to bottom, highest abstraction at the top; notes in a right-hand column | layers of different widths |
| medallion | three or four tier zones left to right (bronze, silver, gold); table nodes inside; access rules as free text under each tier | more than 4 fill colours |
| exploded | parts as isometric `line` polygons offset along one diagonal; dashed guide lines between the parts | parts that overlap |
| axonometric-plan | floor and rooms as `line` polygons in isometric projection: `x' = (x - y) * 0.866`, `y' = (x + y) * 0.5` | rooms computed by eye |

## Flow, process, and behaviour

| Type | Build | Look for |
|---|---|---|
| flowchart | one top-down column; step `rectangle`s, decision `diamond`s; "Yes"/"No" labels on the branch arrows | a branch that crosses the main column |
| process | numbered step nodes in a row; actor names above or beside; hand-off arrows labelled with the data | unnumbered steps |
| data-flow | role lanes as horizontal zones; steps inside; arrows labelled with the data; fan-in queue as 3 stacked offset rectangles | unlabelled data arrows |
| swimlane | full-width lane zones with the lane name as free text at the left; steps inside lanes; routed elbow arrows between lanes | hand-offs drawn as diagonals |
| sequence | actor nodes along the top; dashed vertical `line` lifelines; messages as routed horizontal arrows at increasing y; returns dashed | messages that are not horizontal |
| state | states as rectangles with `roundness: {type: 3}`; start = filled 16px dot; end = dot inside a ring; arrows labelled `event [guard]` | a state without an exit that is not an end state |
| loop | 5 to 8 stations on a circle (compute with cos/sin), hub node in the centre; station-to-station arrows routed with one mid waypoint and `roundness: {type: 2}`; dashed spokes to the hub | spokes without the dashed style |
| service-blueprint | horizontal bands: customer actions, frontstage, backstage, support; a dashed `line` as the line of visibility; one column per stage | a backstage step above the line |
| journey | stage columns with headers; an actions row; an emotion curve as one `line` whose y follows the sentiment; pain points negative | a straight emotion line |
| fishbone | spine `arrow` to an effect node at the right; category bones as `line`s at 60 degrees; causes as free text along the bones | causes not on a bone |
| tree | one row per depth; each parent centred over its children; routed elbow connectors | sibling rows at different heights |
| org-chart | the tree recipe; person nodes with two lines (name, role); dotted-line reporting as dashed connectors | role and name on one line |
| story-map | backbone row of activity nodes; story cards in columns under each activity; dashed horizontal release lines with free-text labels | a story outside its column |
| kanban | one column zone per state, with the header and WIP limit as free text; cards stacked; blocked cards negative | a column over its WIP limit without a mark |
| timeline | one horizontal `line`; 16px dots at the event x; labels alternate above and below | labels that overlap |
| gantt | time ticks as mono free text and dashed vertical grid `line`s; one task bar per row, x = start, width = duration; milestones as small diamonds | bars not aligned to the ticks |

## Data and quantity

Every type in this group is **parametric** (`SKILL.md` step 3).

| Type | Build | Look for |
|---|---|---|
| bar | axis `line`s; bars as `rectangle`s with height in proportion to the value; value labels above; one accent bar | a value axis that does not start at 0 |
| waterfall | each bar starts at the running total; positive and negative styles for up and down; muted totals; dashed `line`s link the bar tops | bars that do not float |
| line | axes; each series one `line` with `points`; label at the end of the line instead of a legend; slopegraph = two vertical axes | a legend where end labels fit |
| scatter | axes; points as dots; bubble = ellipse size in proportion to the third value; label outliers only | labels on every point |
| heatmap | grid of cells, one hue, `opacity` scaled from 10 to 100 by value; row and column labels as free text | more than one hue |
| treemap | squarified tiles (compute them); fill by group; label inside when it fits | tiles computed by eye |
| sankey | node bars as thin vertical rectangles; flows as `line`s with 3 or more `points`, `strokeWidth` in proportion to the amount, `opacity: 50` | band widths that do not add up at a node |
| radar | 3 or 4 concentric closed `line` polygons as the scale; spokes; each entity one closed polygon with `opacity: 30` | more than 3 entities |
| polar | spokes; each category a `line` from the centre with a dot at its radius (lollipop) | radius not in proportion to the value |
| pyramid | stacked trapezoids as closed `line` polygons, centred; funnel = the same shape inverted with % labels | tiers of equal width |
| quadrant | two crossing axis `line`s; axis names at the ends; quadrant names muted in the corners; items as dots with labels | items on an axis line |
| venn | 2 or 3 `ellipse`s with `opacity: 30` that overlap; region labels as free text | a label outside its region |
| wardley | x axis: evolution with 4 stage labels; y axis: visibility; components as dots with labels; dependency `line`s; movement as dashed negative arrows | x axis without stage labels |

## Data models and matrices

| Type | Build | Look for |
|---|---|---|
| er | one rectangle per entity: name as bound text at the top, attributes as left-aligned free text; relation arrows labelled with cardinality (`1..*`) | cardinality missing |
| db-schema | the er recipe with columns in mono text: `name  type  PK/FK`; FK arrows leave from the FK column's y | arrows that do not start at a column |
| uml-class | one rectangle in three parts (name, attributes, operations) split by `line`s; inheritance arrow with `endArrowhead: "triangle"` | operations and attributes mixed |
| dp-security-matrix | grid of cells: roles in rows, components in columns; mono text `R`, `W`, `-` in each cell; fill by access level | more than 3 fill levels |
