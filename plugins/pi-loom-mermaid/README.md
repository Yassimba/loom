# pi-loom-mermaid

Draws Mermaid blocks in Pi as Unicode terminal diagrams. Colored borders, hops where edges cross, and fewer stacked rails than Pi's built-in renderer or pi-lovely-mermaid. It also adds a short system prompt so the agent can answer in diagrams when that is clearer than prose.

Same diagram:

Pi built-in. One gray box style. Edges share long outer rails.

<p align="center">
  <img src="../../assets/mermaid-pi-builtin.png" alt="Pi built-in Mermaid: gray boxes and stacked outer edges">
</p>

pi-lovely-mermaid. Color, still long parallel paths around the right side.

<p align="center">
  <img src="../../assets/mermaid-lovely.png" alt="pi-lovely-mermaid: colored borders with long parallel edges">
</p>

This package. The same colors, plus crossing hops and shorter routes.

<p align="center">
  <img src="../../assets/mermaid-pi-loom.png" alt="pi-loom-mermaid: colored borders, crossing hops, and shorter routes">
</p>

GitHub draws the next blocks with its own Mermaid. Paste the same source into Pi (built-in vs this package) to compare routing and color.

```mermaid
flowchart TD
    CLI["turbine CLI<br/>shell.main"]:::orange
    LSP["Editor<br/>turbine-lsp"]:::orange
    HTTP["HTTP client"]:::red

    CLI --> SELECT["Select ProjectLayout"]:::orange
    LSP --> WORKSPACE["Discover Projects<br/>EditorWorkspace.open"]:::orange

    SELECT --> RUNTIME["ProjectRuntime.create"]:::orange
    WORKSPACE --> RUNTIME

    ENTRY["Installed turbine.extension<br/>entry points"]:::red --> EXT["Discover, order, and admit<br/>Extensions"]:::orange
    RUNTIME --> EXT
    EXT --> CATALOG["ExtensionCatalog"]:::green
    CATALOG --> FORMATS["InstalledFormats"]:::green
    CATALOG --> LINT["CachedProjectLint"]:::green

    RUNTIME --> SNAPSHOT["ProjectSnapshotCache"]:::green
    RUNTIME --> RUN["CheckRun"]:::green
    RUNTIME --> HISTORY["RunHistoryReader"]:::green

    SNAPSHOT --> COMMANDS["CLI commands"]:::orange
    SNAPSHOT --> SESSION["EditorSession"]:::orange
    SNAPSHOT --> API["Management API"]:::orange

    CLI --> COMMANDS
    LSP --> SESSION
    HTTP --> API

    classDef red stroke:#9f5555
    classDef orange stroke:#9a7438
    classDef green stroke:#4f8560
```

Stress cases: complex diagrams rendered by both engines.

**State — Pi built-in**

<p align="center">
  <img src="../../assets/mermaid-state-pi.png" alt="Pi built-in state diagram">
</p>

**State — pi-loom-mermaid**

<p align="center">
  <img src="../../assets/mermaid-state-loom.png" alt="pi-loom-mermaid state diagram">
</p>

**ER — Pi built-in**

<p align="center">
  <img src="../../assets/mermaid-er-pi.png" alt="Pi built-in ER diagram">
</p>

**ER — pi-loom-mermaid**

<p align="center">
  <img src="../../assets/mermaid-er-loom.png" alt="pi-loom-mermaid ER diagram">
</p>

**Sequence — Pi built-in**

<p align="center">
  <img src="../../assets/mermaid-sequence-pi.png" alt="Pi built-in sequence diagram">
</p>

**Sequence — pi-loom-mermaid**

<p align="center">
  <img src="../../assets/mermaid-sequence-loom.png" alt="pi-loom-mermaid sequence diagram">
</p>

**Dense class graph — Pi built-in**

<p align="center">
  <img src="../../assets/mermaid-class-pi.png" alt="Pi built-in dense class diagram">
</p>

**Dense class graph — pi-loom-mermaid**

<p align="center">
  <img src="../../assets/mermaid-class-loom.png" alt="pi-loom-mermaid dense class diagram">
</p>

**Dense flowchart — Pi built-in**

<p align="center">
  <img src="../../assets/mermaid-dense-pi.png" alt="Pi built-in dense flowchart">
</p>

**Dense flowchart — pi-loom-mermaid**

<p align="center">
  <img src="../../assets/mermaid-dense-loom.png" alt="pi-loom-mermaid dense flowchart">
</p>

## Install

From Loom setup, or:

```bash
pi install npm:@yassimba/pi-loom-mermaid
```

Turn off Pi's built-in Mermaid transformer so this extension gets the original source. In `~/.pi/agent/settings.json`:

```json
{
  "markdown": {
    "mermaid": "off"
  }
}
```

Then run `/reload` in Pi.

## Usage

Mark diff nodes with the built-in `red`, `orange`, and `green` classes:

```mermaid
flowchart LR
  A[Removed]:::red --> B[Changed]:::orange --> C[Added]:::green
```

Those classes color and dim only the box border. Fill and text stay on Pi's theme. A `classDef` in the diagram overrides a built-in class or adds other colors.

Diagrams wider than the terminal stay as Mermaid source.

## License

[MIT](LICENSE)

pi-loom-mermaid is adapted from pi-lovely-mermaid.
