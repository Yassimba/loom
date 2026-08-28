# HTML Presentation Template

Reference architecture for generating slide presentations. Every presentation follows a fixed 16:9 stage model: slides are authored at 1920×1080 and the whole stage scales to fit the browser window.

## Base HTML Structure

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Presentation Title</title>

    <!-- Fonts: use Fontshare or Google Fonts — never system fonts -->
    <link rel="stylesheet" href="https://api.fontshare.com/v2/css?f[]=...">

    <style>
        /* ===========================================
           CSS CUSTOM PROPERTIES (THEME)
           Change these to change the whole look
           =========================================== */
        :root {
            /* Colors — from chosen style preset */
            --bg-primary: #0a0f1c;
            --bg-secondary: #111827;
            --text-primary: #ffffff;
            --text-secondary: #9ca3af;
            --accent: #00ffcc;
            --accent-glow: rgba(0, 255, 204, 0.3);

            /* Typography — authored at 1920×1080 stage size */
            --font-display: 'Clash Display', sans-serif;
            --font-body: 'Satoshi', sans-serif;
            --title-size: 112px;
            --subtitle-size: 34px;
            --body-size: 28px;

            /* Spacing — authored at 1920×1080 stage size */
            --slide-padding: 72px;
            --content-gap: 32px;

            /* Animation */
            --ease-out-expo: cubic-bezier(0.16, 1, 0.3, 1);
            --duration-normal: 0.6s;
        }

        /* ===========================================
           BASE STYLES
           =========================================== */
        * { margin: 0; padding: 0; box-sizing: border-box; }

        /* --- PASTE viewport-base.css CONTENTS HERE --- */

        /* ===========================================
           ANIMATIONS
           Trigger via .visible class on the active slide
           =========================================== */
        .reveal {
            opacity: 0;
            transform: translateY(30px);
            transition: opacity var(--duration-normal) var(--ease-out-expo),
                        transform var(--duration-normal) var(--ease-out-expo);
        }

        .slide.visible .reveal {
            opacity: 1;
            transform: translateY(0);
        }

        /* Stagger children for sequential reveal */
        .reveal:nth-child(1) { transition-delay: 0.1s; }
        .reveal:nth-child(2) { transition-delay: 0.2s; }
        .reveal:nth-child(3) { transition-delay: 0.3s; }
        .reveal:nth-child(4) { transition-delay: 0.4s; }

        /* ... preset-specific styles ... */
    </style>
</head>
<body>
    <div class="deck-viewport">
        <main class="deck-stage" id="deckStage">
            <section class="slide title-slide active">
                <h1 class="reveal">Presentation Title</h1>
                <p class="reveal">Subtitle or author</p>
            </section>

            <section class="slide">
                <div class="slide-content">
                    <h2 class="reveal">Slide Title</h2>
                    <p class="reveal">Content...</p>
                </div>
            </section>

            <!-- More slides... -->
        </main>
    </div>

    <script>
        /* ===========================================
           SLIDE PRESENTATION CONTROLLER
           =========================================== */
        class SlidePresentation {
            constructor() {
                this.slides = document.querySelectorAll('.slide');
                this.currentSlide = 0;
                this.stage = document.getElementById('deckStage');
                this.setupStageScale();
                this.setupKeyboardNav();
                this.setupTouchNav();
                this.showSlide(0);
            }

            setupStageScale() {
                const scale = () => {
                    const factor = Math.min(window.innerWidth / 1920, window.innerHeight / 1080);
                    const x = (window.innerWidth - 1920 * factor) / 2;
                    const y = (window.innerHeight - 1080 * factor) / 2;
                    this.stage.style.transform = `translate(${x}px, ${y}px) scale(${factor})`;
                };
                scale();
                window.addEventListener('resize', scale);
            }

            setupKeyboardNav() {
                // Arrow keys, Space, Page Up/Down
            }

            setupTouchNav() {
                // Touch/swipe support for mobile
            }

            showSlide(index) {
                this.currentSlide = Math.max(0, Math.min(index, this.slides.length - 1));
                this.slides.forEach((slide, i) => {
                    slide.classList.toggle('active', i === this.currentSlide);
                    slide.classList.toggle('visible', i === this.currentSlide);
                });
            }
        }

        new SlidePresentation();
    </script>
</body>
</html>
```

## Required JavaScript Features

Every presentation must include:

1. **SlidePresentation Class** — Main controller with:
   - Keyboard navigation (arrows, space, page up/down)
   - Touch/swipe support
   - Mouse wheel navigation
   - Optional progress indicator or page count, kept outside the slide stage

2. **Stage Scaling** — For fixed 16:9 presentation behavior:
   - Keep all slides at 1920×1080 inside `.deck-stage`
   - Scale the whole stage with one transform
   - Letterbox/pillarbox as needed; never reflow slide content per device

3. **Optional Enhancements** (match to chosen style):
   - Custom cursor with trail
   - Particle system background (canvas)
   - Parallax effects
   - 3D tilt on hover
   - Magnetic buttons
   - Counter animations

4. **Inline Editing** (included by default after draft generation):
   - Edit toggle button (hidden by default, revealed via hover hotzone or `E` key)
   - Auto-save to localStorage
   - Export/save file functionality
   - See "Inline Editing Implementation" section below

## Inline Editing Implementation

Inline editing is a lightweight post-draft affordance. Do not ask the user whether they want it during the pre-generation Q&A. Include it by default unless the user explicitly asks for a locked/export-only presentation or no editing controls.

**Do NOT use CSS `~` sibling selector for hover-based show/hide.** The CSS-only approach (`edit-hotzone:hover ~ .edit-toggle`) fails because `pointer-events: none` on the toggle button breaks the hover chain: user hovers hotzone -> button becomes visible -> mouse moves toward button -> leaves hotzone -> button disappears before click.

**Required approach: JS-based hover with 400ms delay timeout.**

HTML:
```html
<div class="edit-hotzone"></div>
<button class="edit-toggle" id="editToggle" title="Edit mode (E)">✏️</button>
```

CSS (visibility controlled by JS classes only):
```css
/* Do NOT use CSS ~ sibling selector for this!
   pointer-events: none breaks the hover chain.
   Must use JS with delay timeout. */
.edit-hotzone {
    position: fixed; top: 0; left: 0;
    width: 80px; height: 80px;
    z-index: 10000;
    cursor: pointer;
}
.edit-toggle {
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.3s ease;
    z-index: 10001;
}
.edit-toggle.show,
.edit-toggle.active {
    opacity: 1;
    pointer-events: auto;
}
```

JS (three interaction methods):
```javascript
// 1. Click handler on the toggle button
document.getElementById('editToggle').addEventListener('click', () => {
    editor.toggleEditMode();
});

// 2. Hotzone hover with 400ms grace period
const hotzone = document.querySelector('.edit-hotzone');
const editToggle = document.getElementById('editToggle');
let hideTimeout = null;

hotzone.addEventListener('mouseenter', () => {
    clearTimeout(hideTimeout);
    editToggle.classList.add('show');
});
hotzone.addEventListener('mouseleave', () => {
    hideTimeout = setTimeout(() => {
        if (!editor.isActive) editToggle.classList.remove('show');
    }, 400);
});
editToggle.addEventListener('mouseenter', () => {
    clearTimeout(hideTimeout);
});
editToggle.addEventListener('mouseleave', () => {
    hideTimeout = setTimeout(() => {
        if (!editor.isActive) editToggle.classList.remove('show');
    }, 400);
});

// 3. Hotzone direct click
hotzone.addEventListener('click', () => {
    editor.toggleEditMode();
});

// 4. Keyboard shortcut (E key, skip when editing text)
document.addEventListener('keydown', (e) => {
    if ((e.key === 'e' || e.key === 'E') && !e.target.getAttribute('contenteditable')) {
        editor.toggleEditMode();
    }
});
```

## Component Vocabulary

Four components appear in nearly every technical deck. Each is written once here because each carries a trap that only a screenshot reveals.

### Code and terminal blocks

**`white-space` is the whole component.** A block written with literal newlines and no `white-space` collapses every line into one run of text. The markup looks correct, the render is ruined, and nothing but a screenshot catches it.

```css
.code,
.term {
    background: var(--bg-sunken);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 24px 28px;
    font-family: var(--font-mono);
    font-size: 18px;          /* the floor — never smaller on a 1920 stage */
    line-height: 1.65;
    white-space: pre;         /* `pre-wrap` when long lines must wrap instead of clip */
    overflow: hidden;
    tab-size: 2;
}
.code { border-left: 2px solid var(--accent); }

/* Syntax tones: keyword, string, number, comment, emphasis.
   Five is enough — more reads as confetti at presentation distance. */
.code .k  { color: var(--accent); }
.code .s  { color: var(--ok, #3FCF8E); }
.code .n  { color: #E5C07B; }
.code .c  { color: var(--fg-3); font-style: italic; }
.code .hl { color: var(--fg-1); font-weight: 500; }

/* A terminal earns its frame by showing a prompt and real result colours. */
.term .cmd::before { content: "$ "; color: var(--accent); }
.term .ok { color: var(--ok, #3FCF8E); }
.term .no { color: var(--bad, #FF5F56); }
```

### Table

```css
.tbl { width: 100%; border-collapse: collapse; font-size: 19px; }
.tbl th {
    font-family: var(--font-label);
    font-size: 18px; font-weight: 400;    /* the floor applies to headers too */
    letter-spacing: 0.14em; text-transform: uppercase;
    color: var(--fg-2);                   /* headers are text: keep them above the AA floor */
    text-align: left;
    padding: 0 20px 14px 0;
    border-bottom: 1px solid var(--line-strong);
}
.tbl td {
    padding: 14px 20px 14px 0;
    border-bottom: 1px solid var(--line-soft);
    vertical-align: top; line-height: 1.4;
}
.tbl tr:last-child td { border-bottom: none; }
```

A table past 8 rows or 5 columns stops being readable at presentation distance — split it across two slides, or turn it into a figure.

### Callout

```css
.note {
    border-left: 2px solid var(--accent);
    background: var(--bg-raised);
    border-radius: 0 4px 4px 0;
    padding: 15px 22px;
    display: flex; gap: 18px; align-items: flex-start;
}
.note-label {
    font-family: var(--font-label);
    font-size: 18px; letter-spacing: 0.14em; text-transform: uppercase;
    color: var(--accent); white-space: nowrap; padding-top: 2px;
}
.note p { font-size: 19px; line-height: 1.45; }

/* Inline code inherits a browser default near 13px — always set it back.
   `em` compounds inside small parents, so pin it in px. */
code {
    font-family: var(--font-mono);
    font-size: 18px;
    background: var(--bg-sunken);
    border: 1px solid var(--line-soft);
    border-radius: 4px;
    padding: 1px 6px;
}
```

The label names the *kind* of aside in one word — `RULE`, `TRAP`, `CHECKPOINT`, `WHY` — so the reader classifies it before reading it.

### Type floor

Every component above sits at 18px or larger, and that is the floor for the whole stage. Three places leak below it by default and need pinning every time: inline `<code>` (a browser default near 13px), table headers styled as small caps, and text inside a callout that inherits from a smaller parent. When a block will not fit at 18px, the content is too long for one slide: split it. Shrinking to 15px reliably produces a slide nobody in the third row can read.

## Presenter Mode

Speaker notes live in the deck and open in a second window, so the deck stays one file and the audience never sees the notes.

```html
<section class="slide">
    <div class="slide-content">…</div>
    <aside class="notes" hidden>
        Open with the failing build. Ask who has seen this before — usually half the room.
        Land on the one-line fix before advancing.
    </aside>
</section>
```

`hidden` keeps notes out of the slide, out of the print/PDF export, and out of the checker's measurements.

```javascript
/* ===========================================
   PRESENTER MODE — press P
   A second window shows the current note, the next slide's note,
   elapsed time, and the slide counter. It follows the main window.
   =========================================== */
class Presenter {
    constructor(deck) {
        this.deck = deck;
        this.win = null;
        this.start = null;
    }

    toggle() {
        if (this.win && !this.win.closed) { this.win.close(); this.win = null; return; }
        this.win = window.open('', 'presenter', 'width=900,height=650');
        if (!this.win) return;               // popup blocked; stay silent and keep presenting
        this.start = Date.now();
        this.win.document.write(`<!doctype html><meta charset="utf-8"><title>Presenter</title>
            <style>
              body{margin:0;padding:28px;font:16px/1.6 system-ui,sans-serif;
                   background:#0b0d12;color:#e8eaf0}
              .row{display:flex;justify-content:space-between;align-items:baseline;
                   border-bottom:1px solid #262a35;padding-bottom:12px;margin-bottom:20px}
              .time{font-size:34px;font-variant-numeric:tabular-nums}
              .count{color:#868ea1}
              h2{font-size:13px;letter-spacing:.14em;text-transform:uppercase;
                 color:#868ea1;margin:24px 0 8px}
              .now{font-size:20px;white-space:pre-wrap}
              .next{color:#868ea1;white-space:pre-wrap}
            </style>
            <div class="row"><span class="time" id="t">00:00</span>
                             <span class="count" id="c"></span></div>
            <h2>This slide</h2><div class="now" id="now"></div>
            <h2>Next</h2><div class="next" id="next"></div>`);
        this.win.document.close();
        this.tick = setInterval(() => this.render(), 500);
        this.render();
    }

    render() {
        if (!this.win || this.win.closed) { clearInterval(this.tick); this.win = null; return; }
        const d = this.win.document;
        const noteAt = (i) => {
            const s = this.deck.slides[i];
            const n = s && s.querySelector('.notes');
            return n ? n.textContent.trim() : '';
        };
        const i = this.deck.currentSlide ?? this.deck.current ?? 0;
        const secs = Math.floor((Date.now() - this.start) / 1000);
        d.getElementById('t').textContent =
            String(Math.floor(secs / 60)).padStart(2, '0') + ':' + String(secs % 60).padStart(2, '0');
        d.getElementById('c').textContent = `${i + 1} / ${this.deck.slides.length}`;
        d.getElementById('now').textContent = noteAt(i) || '—';
        d.getElementById('next').textContent = noteAt(i + 1) || '—';
    }
}

const presenter = new Presenter(deck);
document.addEventListener('keydown', (e) => {
    if ((e.key === 'p' || e.key === 'P') &&
        e.target.getAttribute('contenteditable') !== 'true') presenter.toggle();
});
```

Call `presenter.render()` at the end of the deck's `showSlide()` so the second window advances with the first. Tell the user about `P` in the Phase 5 summary whenever the deck has notes.

## Image Pipeline (Skip If No Images)

Skip this section unless the user supplied an image folder. When they did, process the images before generating the HTML.

**Dependency:** `pip install Pillow`

### Image Processing

```python
from PIL import Image, ImageDraw

# Circular crop (for logos on modern/clean styles)
def crop_circle(input_path, output_path):
    img = Image.open(input_path).convert('RGBA')
    w, h = img.size
    size = min(w, h)
    left, top = (w - size) // 2, (h - size) // 2
    img = img.crop((left, top, left + size, top + size))
    mask = Image.new('L', (size, size), 0)
    ImageDraw.Draw(mask).ellipse([0, 0, size, size], fill=255)
    img.putalpha(mask)
    img.save(output_path, 'PNG')

# Resize (for oversized images that inflate HTML)
def resize_max(input_path, output_path, max_dim=1200):
    img = Image.open(input_path)
    img.thumbnail((max_dim, max_dim), Image.LANCZOS)
    img.save(output_path, quality=85)
```

| Situation | Operation |
|-----------|-----------|
| Square logo on rounded aesthetic | `crop_circle()` |
| Image > 1MB | `resize_max(max_dim=1200)` |
| Wrong aspect ratio | Manual crop with `img.crop()` |

Save processed images with `_processed` suffix. Never overwrite originals.

### Image Placement

**Use direct file paths** (not base64) — presentations are viewed locally:

```html
<img src="assets/logo_round.png" alt="Logo" class="slide-image logo">
<img src="assets/screenshot.png" alt="Screenshot" class="slide-image screenshot">
```

```css
.slide-image {
    max-width: 100%;
    max-height: min(50vh, 400px);
    object-fit: contain;
    border-radius: 8px;
}
.slide-image.screenshot {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}
.slide-image.logo {
    max-height: min(30vh, 200px);
}
```

**Adapt border/shadow colors to match the chosen style's accent.** Never repeat the same image on multiple slides (except logos on title + closing).

**Placement patterns:** Logo centered on title slide. Screenshots in two-column layouts with text. Full-bleed images as slide backgrounds with text overlay (use sparingly).

---

## Code Quality

**Comments:** Every section needs clear comments explaining what it does and how to modify it.

**Accessibility:**
- Semantic HTML (`<section>`, `<nav>`, `<main>`)
- Keyboard navigation works fully
- ARIA labels where needed
- `prefers-reduced-motion` support (included in viewport-base.css)

## File Structure

Single presentations:
```
presentation.html    # Self-contained, all CSS/JS inline
assets/              # Images only, if any
```

Multiple presentations in one project:
```
[name].html
[name]-assets/
```
