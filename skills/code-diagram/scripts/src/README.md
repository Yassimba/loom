# Code diagram source

The compiler follows one path from authored input to offline HTML:

```text
code-diagram.ts
  -> document/registry.ts
  -> diagrams/<type>/descriptor.ts
  -> diagrams/<type>/viewer.tsx
  -> viewer/mount.tsx
```

## Add a diagram type

1. Put its authoring model, compiler, renderer, and descriptor in `diagrams/<type>/`.
2. Export `schema` and `Renderer` from its viewer module.
3. Register its descriptor in `document/registry.ts`.
4. Add an isolated surface case to the conformance test in `test/code-diagram.test.ts`.

The compiler bundles only registered types used by the document. Declare `assets: ["libavoid"]` only when the renderer uses the shared orthogonal canvas.

## Folders

- `authoring/` contains shared references and software-map-backed actor/store definitions. Diagram-owned authoring schemas stay with their diagram.
- `document/` evaluates artifact modules, resolves source evidence, parses patches and counts changed lines, compiles diagrams, and defines the renderer and output contracts.
- `diagrams/` owns each diagram type's descriptor, authoring grammar, compiler, renderer, and styles.
- `canvas/` contains the diagram-neutral scene model, interaction state, C4 canvas grammar, and routing machinery used by Software Map and Database Lens.
- `viewer/` mounts compiled diagrams and provides shared browser styles and source events.

## Canvas seam

Software Map and Database Lens adapt their typed models to `DiagramSnapshot`:

```text
Software Map  -> software-map/c4-adapter.ts ----\
                                                 -> DiagramCanvas -> C4 layout -> routing
Database Lens -> databaseDiagramSnapshot() -----/
```

`canvas/c4/canvas.tsx` owns the shared C4 grammar. `canvas/routing/route.ts` owns connector routing, label placement, and fallback behavior. Diagram adapters own semantic projection and node placement policy.

Keep sequence, timeline, matrix, and statistical renderers outside ReactFlow unless their grammar genuinely needs a node-and-relationship canvas.

## Shared non-layout seams

- `document/artifact.ts` bundles, imports, and cleans up typed artifact modules; descriptors provide aliases and validate the exported value.
- `document/anchor-evidence.ts` resolves peekable anchors without losing diagram-specific fields.
- `document/diff.ts` owns generic patch and changed-line processing.
- `DiagramRendererProps<Model>` in `document/model.ts` is the browser renderer boundary.
- `canvas/data-store-schema.ts` owns datastore field traversal shared by Software Map and Database Lens.

Do not add a universal layout or ReactFlow host: renderers continue to own layout, measurement, and viewport policy.
