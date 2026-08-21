# Pandoc Setup for WEAVE Output

## Prerequisites

### Install Pandoc

```bash
# macOS
brew install pandoc

# Ubuntu / Debian
sudo apt-get install pandoc

# Or download from https://pandoc.org/installing.html
```

### Install a LaTeX Distribution (for PDF output)

```bash
# macOS
brew install --cask mactex-no-gui
# or the smaller: brew install basictex

# Ubuntu / Debian
sudo apt-get install texlive-xetex texlive-fonts-recommended texlive-fonts-extra
```

### Install mermaid-filter (for diagrams)

```bash
bun install -g mermaid-filter
# or: npm install -g mermaid-filter
```

This requires Chromium/Chrome for rendering. If running headless, also install `puppeteer`:

```bash
bun install -g puppeteer
```

## Generating a PDF

From the directory containing your `.lit.md` file:

```bash
pandoc project.lit.md \
  -o project.pdf \
  --pdf-engine=xelatex \
  --filter mermaid-filter \
  --toc \
  --number-sections \
  -V geometry:"left=0.75in, right=1in, top=1in, bottom=1in" \
  -V fontsize=11pt
```

### With the Pandoc Header Template

If your `.lit.md` includes the YAML frontmatter from `assets/pandoc-header.yaml`, you can simplify:

```bash
pandoc project.lit.md \
  -o project.pdf \
  --pdf-engine=xelatex \
  --filter mermaid-filter
```

The frontmatter handles TOC, fonts, geometry, and other settings.

## Troubleshooting

### "xelatex not found"
Ensure your LaTeX distribution is on `$PATH`. On macOS after installing MacTeX:
```bash
eval "$(/usr/libexec/path_helper)"
```

### mermaid diagrams not rendering
- Verify: `which mermaid-filter` returns a path
- Check Chrome/Chromium is installed: `which chromium || which google-chrome`
- For headless environments, set: `export PUPPETEER_EXECUTABLE_PATH=/usr/bin/chromium`

### "Missing font" errors
Either install the fonts specified in the YAML frontmatter, or change `mainfont` and `monofont` to fonts available on your system. List available fonts with:
```bash
fc-list | grep -i "fira"
```

### Large documents timing out
Add `--resource-path=.` and increase the mermaid timeout:
```bash
MERMAID_FILTER_TIMEOUT=60000 pandoc project.lit.md -o project.pdf ...
```

## TikZ as Mermaid Alternative

Use TikZ when `mermaid-filter` is unavailable or undesired (e.g., CI environments without Chrome, or minimal installs).

### When to use it

- `which mermaid-filter` returns nothing
- You want diagrams that render without any Node.js tooling
- You need precise control over diagram layout

### Installation

TikZ is included in standard TeX distributions. If LaTeX is already installed for PDF output, no extra installation is needed:

```bash
# Verify TikZ is available (should print the .sty path)
kpsewhich tikz.sty
```

The `assets/pandoc-header.yaml` template already loads TikZ and the `shapes`, `arrows`, `automata`, and `positioning` libraries via `header-includes`.

### Usage

Write diagrams as pandoc raw LaTeX blocks so they pass through to xelatex:

````markdown
```{=latex}
\begin{tikzpicture}[...]
  ...
\end{tikzpicture}
```
````

These blocks are invisible in Markdown preview but render correctly in the PDF. See `SKILL.md` for TikZ translation patterns for common Mermaid diagram types.
