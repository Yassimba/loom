# Domain modeling during a grill

Load this reference only when the discussion defines project-specific language or settles an enduring architectural decision.

## Locate the domain docs

Most repos use a root `CONTEXT.md` and system-wide ADRs in `ai-docs/adr/`. If `CONTEXT-MAP.md` exists, use it to find each context's `CONTEXT.md` and context-local `ai-docs/adr/` directory.

Create files lazily: the first resolved term creates `CONTEXT.md`; the first qualifying decision creates `ai-docs/adr/`.

## During the session

- Call out conflicts with an existing glossary immediately.
- Replace vague or overloaded language with one canonical project term.
- Probe domain boundaries with concrete edge-case scenarios.
- Check stated behavior against the code and surface contradictions.
- Update the relevant `CONTEXT.md` as soon as a term is resolved.

`CONTEXT.md` is a glossary, not a spec or implementation guide. Use this shape:

```md
# {Context Name}

{One or two sentences describing the context.}

## Language

**Order**:
{One or two sentences defining the term.}
_Avoid_: Purchase, transaction
```

Choose one term, list rejected synonyms under `_Avoid_`, and include only concepts specific to this project's domain. Group terms only when natural clusters emerge.

For multiple contexts, maintain a root `CONTEXT-MAP.md`:

```md
# Context Map

## Contexts

- [Ordering](./src/ordering/CONTEXT.md): receives and tracks customer orders

## Relationships

- **Ordering → Fulfillment**: emits `OrderPlaced`; Fulfillment starts picking
```

## ADR check

Offer an ADR only when the decision is all three:

1. Hard to reverse
2. Surprising without context
3. The result of a real trade-off

ADRs live at `ai-docs/adr/NNNN-slug.md`; scan for the highest number and increment it. Keep the default format to one short document:

```md
# {Short title}

{What was decided, why, and the context needed to understand it.}
```

Add status, alternatives, or consequences only when they carry information a future reader needs.
