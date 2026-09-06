# Colored Mermaid diagrams for Pi

Render Mermaid blocks as Unicode terminal diagrams with per-node colors.

## Install

```bash
pi install npm:@yassimba/pi-loom-mermaid
```

Disable Pi's built-in Mermaid transformer in `~/.pi/agent/settings.json` so this extension receives the original Mermaid source:

```json
{
  "markdown": {
    "mermaid": "off"
  }
}
```

Run `/reload`, then mark diff nodes with the built-in `red`, `orange`, and `green` classes:

```mermaid
flowchart LR
  A[Removed]:::red --> B[Changed]:::orange --> C[Added]:::green
```

The built-in classes dim and color only the box border; backgrounds and text keep Pi's normal theme. Standard Mermaid `classDef` declarations can override these defaults or add other colors. Diagrams wider than the terminal remain Mermaid source blocks.
