# Plannotator slide review

Create a temporary sibling of the finished deck named `<stem>-annotate.html`. Keep the delivered HTML unchanged. In the temporary copy, append this final style block before `</head>`:

```html
<style id="plannotator-slide-overview">
html, body { height: auto !important; overflow: auto !important; }
.deck-viewport {
  position: static !important;
  width: 1920px !important;
  height: auto !important;
  overflow: visible !important;
  zoom: .5;
}
.deck-stage {
  position: static !important;
  width: 1920px !important;
  height: auto !important;
  transform: none !important;
  display: flex !important;
  flex-direction: column !important;
  gap: 48px !important;
}
.slide {
  position: relative !important;
  inset: auto !important;
  width: 1920px !important;
  height: 1080px !important;
  visibility: visible !important;
  opacity: 1 !important;
  pointer-events: auto !important;
}
.slide .reveal { opacity: 1 !important; transform: none !important; }
</style>
```

Open the temporary copy with `plannotator annotate <stem>-annotate.html --json`, in the background without a timeout. Use `annotate`, not bare `plannotator`: automatic plan review only presents a plan, while raw-HTML annotation preserves the rendered slides.

Before handing it over, verify that every `.slide` is visible and the page scrolls through the complete deck. Apply returned feedback to the delivered HTML, rerun `scripts/check-deck.py`, and regenerate the temporary copy if the user wants another pass. Delete the temporary copy when review is finished.
