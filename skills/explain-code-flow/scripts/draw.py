#!/usr/bin/env python3
"""Public drawing API for explain-code-flow figures.

Import with ``from draw import *``. Compose SVG fragments in paint order
(zones, connectors, labels, nodes, legend), then call ``write``. Coordinates
use the 4px grid. Labels belong on free connector segments.

Nodes
    node, participant, state, start, ring, diamond, oval, step, cls
Connectors
    hline, vline, line, path, elbow, uml
Labels
    label_above, label_beside, mult
Containers
    zone, lifeline, activation, fragment
Chrome
    callout, legend, sw_box, sw_line, sw_uml, sw_ring, sw_start,
    sw_diamond, sw_oval
Output
    page, export_svg, write

``participant`` and ``cls`` return tuples; all other primitives return SVG
strings. ``write(stem, eyebrow, title, desc, width, height, body, project='')``
emits ``stem.html`` and ``stem.svg``. Read ``example-figure.py`` for one worked
composition. The private implementation is not authoring guidance.
"""

from _draw_impl import *  # noqa: F401,F403
