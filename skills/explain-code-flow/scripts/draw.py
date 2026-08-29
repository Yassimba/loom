#!/usr/bin/env python3
"""Public drawing API for explain-code-flow figures.

Import with ``from draw import *``. Compose SVG fragments in paint order
(zones, connectors, labels, nodes, legend), then call ``write``. Coordinates
use the 4px grid. Labels belong on free connector segments.

Common signatures
    node(x,y,w,h,name,sub=None,kind="step",tag=None,mono=False)
    participant(x,y,w,h,name,sub=None,kind="step",tag=None)
    state(x,y,w,h,name,sub=None,focal=False,wait=False)
    start(cx,cy); ring(cx,cy,label); diamond(cx,cy,text,focal=False,sub=None)
    oval(x,y,w,h,text,sub=None); step(x,y,w,h,lines,eyebrow=None)
    cls(x,y,w,name,attrs,ops=None,stereotype=None,focal=False)
    hline(x1,y1,x2,y2=None,**style); line(x1,y1,x2,y2,**style)
    elbow([(x,y),...],r=8,**style); path(d,**style); uml(d,...)
    label_above(cx,y,text,lines=None); label_beside(x,cy,text,lines=None)
    zone(x,y,w,h,label,accent=False); lifeline(x,top,bottom)
    activation(x,top,bottom,w=8); fragment(x,y,w,h,op,guard)
    callout(x,y,lines); legend(y,width,items)

Kinds: focal, step, store, external, input, async. Style keys: color,
``dashed``, ``marker``, ``width``. Other primitives: mult and sw_box,
sw_line, sw_uml, sw_ring, sw_start, sw_diamond, sw_oval.
Output: page, export_svg, write.

``participant`` and ``cls`` return tuples; all other primitives return SVG
strings. ``write(stem, eyebrow, title, desc, width, height, body, project='')``
emits ``stem.html`` and ``stem.svg``. Read ``example-figure.py`` for one worked
composition. The private implementation is not authoring guidance.
"""

from _draw_impl import *  # noqa: F401,F403
