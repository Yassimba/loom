#!/usr/bin/env python3
"""Verify a Frontend Slides deck: fit, overlap, legibility, contrast, fill, diagram variety.

Usage:
    uv run --with playwright python scripts/check-deck.py <deck.html> [--shots DIR] [--no-shots]

Exits 0 when the deck is clean, 1 when any slide fails. Every failure names the
slide number, the rule, and the offending element, so the fix is a direct edit.

The one subtlety worth knowing: a slide is measured only after its entrance
animation has settled. Measuring earlier reads the `.reveal` start transform
(typically a 20-30px offset) as real overflow and reports every slide as broken.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

from playwright.sync_api import sync_playwright

STAGE_W, STAGE_H = 1920, 1080
MIN_FONT_PX = 18.0
MIN_CONTRAST = 4.5
SETTLE_TIMEOUT_MS = 2500

# One measuring pass per slide, run inside the page. Returns every violation it
# can see, in stage coordinates, so the caller only formats and counts.
MEASURE_JS = r"""
(index) => {
  const slides = [...document.querySelectorAll('.slide')];
  const slide = slides[index];
  const sb = slide.getBoundingClientRect();
  if (!sb.width) return { skipped: 'slide has no box' };
  const k = 1920 / sb.width;                       // viewport px -> stage px
  const box = (el) => {
    const b = el.getBoundingClientRect();
    return {
      top: (b.top - sb.top) * k, left: (b.left - sb.left) * k,
      bottom: (b.bottom - sb.top) * k, right: (b.right - sb.left) * k,
      w: b.width * k, h: b.height * k,
    };
  };
  const name = (el) => {
    const cls = (el.getAttribute('class') || '').split(/\s+/).filter(Boolean).slice(0, 2).join('.');
    return el.tagName.toLowerCase() + (cls ? '.' + cls : '');
  };
  const text = (el) => (el.textContent || '').trim().replace(/\s+/g, ' ').slice(0, 48);

  // Chrome lives outside the content flow and may sit anywhere on the stage.
  const chrome = (el) => el.closest('.slide-rail, .slide-corner, .deck-controls, .edit-toggle, .edit-hotzone, [data-chrome]');

  const parse = (s) => {
    const m = (s || '').match(/rgba?\(([^)]+)\)/);
    if (!m) return null;
    const p = m[1].split(',').map((n) => parseFloat(n));
    return { rgb: p.slice(0, 3), a: p.length > 3 ? p[3] : 1 };
  };
  const out = { overflow: [], overlap: [], small: [], contrast: [], diagrams: [], underfill: null };
  const els = [...slide.querySelectorAll('*')].filter((el) => {
    if (chrome(el)) return false;
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden' || cs.opacity === '0') return false;
    const b = el.getBoundingClientRect();
    return b.width > 1 && b.height > 1;
  });

  // --- fit: nothing may cross the stage edge ---
  for (const el of els) {
    const b = box(el);
    const over = [];
    if (b.bottom > 1080.5) over.push(`bottom ${Math.round(b.bottom)}`);
    if (b.right > 1920.5) over.push(`right ${Math.round(b.right)}`);
    if (b.top < -0.5) over.push(`top ${Math.round(b.top)}`);
    if (b.left < -0.5) over.push(`left ${Math.round(b.left)}`);
    if (over.length) out.overflow.push({ el: name(el), text: text(el), over: over.join(', ') });
  }

  // --- overlap: two siblings covering each other is the failure screenshots catch
  //     and scrollHeight never does ---
  // Only elements that actually paint can cover one another. A transparent
  // wrapper whose children hold the text overlaps by bounding box constantly
  // and hides nothing, so comparing those produces noise, not findings.
  const paints = (el) => {
    const cs = getComputedStyle(el);
    const bg = parse(cs.backgroundColor);
    if (bg && bg.a > 0.1) return true;
    if (cs.backgroundImage && cs.backgroundImage !== 'none') return true;
    if (parseFloat(cs.borderTopWidth) || parseFloat(cs.borderLeftWidth)) return true;
    return [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
  };
  const parents = new Set(els.map((el) => el.parentElement).filter(Boolean));
  for (const p of parents) {
    const kids = [...p.children].filter((el) => {
      if (!els.includes(el) || !paints(el)) return false;
      const d = getComputedStyle(el).display;
      return d !== 'inline';          // an inline box spanning two lines has a union rect
    });
    for (let i = 0; i < kids.length; i++) {
      for (let j = i + 1; j < kids.length; j++) {
        const a = box(kids[i]), b = box(kids[j]);
        const ax = Math.min(a.right, b.right) - Math.max(a.left, b.left);
        const ay = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        if (ax > 2 && ay > 2) {
          const area = ax * ay, smallest = Math.min(a.w * a.h, b.w * b.h);
          if (smallest > 0 && area / smallest > 0.12) {
            out.overlap.push({ a: name(kids[i]), b: name(kids[j]), pct: Math.round((area / smallest) * 100) });
          }
        }
      }
    }
  }

  // --- legibility: authored px at stage scale ---
  const luminance = (rgb) => {
    const f = rgb.map((v) => { v /= 255; return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; });
    return 0.2126 * f[0] + 0.7152 * f[1] + 0.0722 * f[2];
  };
  const groundOf = (el) => {                      // nearest painted ancestor background
    let n = el;
    while (n && n !== document.documentElement) {
      const c = parse(getComputedStyle(n).backgroundColor);
      if (c && c.a > 0.5) return c.rgb;
      n = n.parentElement;
    }
    return [255, 255, 255];
  };
  const ratio = (fg, bg) => {
    const [a, b] = [luminance(fg), luminance(bg)].sort((x, y) => y - x);
    return (a + 0.05) / (b + 0.05);
  };

  for (const el of els) {
    const hasOwnText = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
    if (!hasOwnText || el.closest('[data-decorative]')) continue;
    const cs = getComputedStyle(el);
    const size = parseFloat(cs.fontSize);
    if (size < 18 - 0.01) out.small.push({ el: name(el), text: text(el), size: Math.round(size * 10) / 10 });
    const fg = parse(cs.color);
    if (fg) {
      const r = ratio(fg.rgb, groundOf(el));
      // Large text carries its own contrast allowance (WCAG: 3:1 at >=24px).
      const floor = size >= 24 ? 3.0 : 4.5;
      if (r < floor) out.contrast.push({ el: name(el), text: text(el), ratio: Math.round(r * 100) / 100, floor });
    }
  }

  // --- diagram variety: each figure declares its form ---
  const figs = [...slide.querySelectorAll('[data-diagram]')].map((el) => el.getAttribute('data-diagram'));
  out.diagrams = figs;
  const figures = [...slide.querySelectorAll('svg:not(.slide-corner):not(.rail-mark)')].filter((el) => {
    const b = box(el);
    return b.w >= 240 && b.h >= 160;   // smaller than this is an icon, not a figure
  });
  out.untaggedSvg = figures.some((el) => !el.hasAttribute('data-diagram'));

  // --- underfill: the body stops short of the bottom margin and the whole
  //     remainder sits in one band beneath the content, which reads as a body
  //     that failed to load. Filling down to the margin passes. Centring --
  //     remainder split evenly above and below -- passes. One lopsided band
  //     does not, and no amount of re-centring fixes it: the slide needs
  //     content, or its content belongs on a slide that has some.
  const frameEl = slide.querySelector('.slide-content');
  if (frameEl) {
    const frame = box(frameEl);
    const marks = els.filter((el) => el !== frameEl && (paints(el) || el.hasAttribute('data-diagram')));
    if (marks.length) {
      const bs = marks.map(box);
      const above = Math.min(...bs.map((b) => b.top)) - frame.top;
      const below = frame.bottom - Math.max(...bs.map((b) => b.bottom));
      if (below > 180 && below - above > 160) {
        out.underfill = { above: Math.round(above), below: Math.round(below) };
      }
    }
  }
  return out;
}
"""

# A slide is settled when every animated descendant has reached its final
# transform and opacity. Polling this beats a fixed sleep: slow font loads and
# long staggers both push the settle point past any constant you would pick.
SETTLED_JS = r"""
(index) => {
  const slide = [...document.querySelectorAll('.slide')][index];
  const moving = [...slide.querySelectorAll('.reveal, [class*="reveal"]')].some((el) => {
    const cs = getComputedStyle(el);
    const t = cs.transform;
    const shifted = t && t !== 'none' && !/matrix\(1, 0, 0, 1, 0, 0\)/.test(t);
    return shifted || parseFloat(cs.opacity) < 0.99;
  });
  return !moving;
}
"""


def activate(page, index: int) -> None:
    """Show one slide, through the deck's own controller when it exposes one."""
    page.evaluate(
        """(i) => {
            if (window.deck && typeof window.deck.show === 'function') { window.deck.show(i); return; }
            document.querySelectorAll('.slide').forEach((s, n) => {
                s.classList.toggle('active', n === i);
                s.classList.toggle('visible', n === i);
            });
        }""",
        index,
    )


def check(deck: pathlib.Path, shots: pathlib.Path | None) -> int:
    failures: list[str] = []
    forms_by_slide: list[list[str]] = []

    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": STAGE_W, "height": STAGE_H})
        page.goto(deck.resolve().as_uri())
        page.wait_for_load_state("networkidle")
        page.evaluate("document.fonts && document.fonts.ready")

        count = page.evaluate("document.querySelectorAll('.slide').length")
        if not count:
            print("FAIL  no .slide elements found — the export and check tooling both key on that class")
            browser.close()
            return 1
        print(f"{deck.name}: {count} slides\n")

        for i in range(count):
            activate(page, i)
            page.wait_for_function(SETTLED_JS, arg=i, timeout=SETTLE_TIMEOUT_MS)
            r = page.evaluate(MEASURE_JS, i)
            n = i + 1

            if r.get("skipped"):
                continue
            for v in r["overflow"]:
                failures.append(f"slide {n:>3}  overflow   {v['el']} past {v['over']}  “{v['text']}”")
            for v in r["overlap"]:
                failures.append(f"slide {n:>3}  overlap    {v['a']} covers {v['pct']}% of {v['b']}")
            for v in r["small"]:
                failures.append(f"slide {n:>3}  small      {v['el']} at {v['size']}px  “{v['text']}”")
            for v in r["contrast"]:
                failures.append(f"slide {n:>3}  contrast   {v['el']} {v['ratio']}:1 (needs {v['floor']})  “{v['text']}”")
            if r.get("untaggedSvg"):
                failures.append(f"slide {n:>3}  untagged   figure has no data-diagram attribute")
            if r.get("underfill"):
                u = r["underfill"]
                failures.append(
                    f"slide {n:>3}  underfill  body stops {u['below']}px above the bottom margin "
                    f"({u['above']}px clear on top) — give the slide content, don't re-centre it"
                )
            forms_by_slide.append(r["diagrams"])

            if shots:
                page.screenshot(path=str(shots / f"slide-{n:02d}.png"))

        # Letterboxing, not reflow, is what a narrow viewport must produce.
        for label, size in (("720p", (1280, 720)), ("phone", (390, 844))):
            page.set_viewport_size({"width": size[0], "height": size[1]})
            activate(page, 0)
            page.wait_for_timeout(400)
            ratio = page.evaluate(
                "() => { const s = document.querySelector('.deck-stage').getBoundingClientRect();"
                " return s.width / s.height; }"
            )
            if abs(ratio - STAGE_W / STAGE_H) > 0.02:
                failures.append(f"viewport {label}  stage is {ratio:.3f}:1, expected 1.778:1 — it reflowed instead of scaling")
            if shots:
                page.screenshot(path=str(shots / f"viewport-{label}.png"))

        browser.close()

    # --- diagram variety, across the deck ---
    diagram_slides = [f for f in forms_by_slide if f]
    flat = [f for forms in diagram_slides for f in forms]
    for a, b in zip(diagram_slides, diagram_slides[1:]):
        shared = set(a) & set(b)
        if shared:
            failures.append(f"variety   {', '.join(sorted(shared))} repeats on consecutive diagram slides")
    if len(diagram_slides) >= 6 and len(set(flat)) < 5:
        failures.append(
            f"variety   {len(diagram_slides)} diagram slides carry only {len(set(flat))} distinct forms "
            f"({', '.join(sorted(set(flat)))}) — six or more need five"
        )

    if failures:
        print("\n".join(failures))
        print(f"\n{len(failures)} problem(s). The deck is not done.")
        return 1

    print(f"clean — fit, overlap, legibility, contrast, vertical fill, letterboxing"
          + (f", and {len(set(flat))} diagram forms across {len(diagram_slides)} figures" if diagram_slides else ""))
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("deck", type=pathlib.Path)
    ap.add_argument("--shots", type=pathlib.Path, default=None, help="directory for screenshots")
    ap.add_argument("--no-shots", action="store_true")
    a = ap.parse_args()

    shots = None
    if not a.no_shots:
        shots = a.shots or a.deck.parent / f"{a.deck.stem}-shots"
        shots.mkdir(parents=True, exist_ok=True)
    return check(a.deck, shots)


if __name__ == "__main__":
    sys.exit(main())
