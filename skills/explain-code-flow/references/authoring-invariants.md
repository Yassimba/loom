# Figure authoring contract

Apply every relevant rule; rendering mechanics live in `draw.py`.

## Shape

- One obvious main path; side branches leave the nearest main-path node.
- Stay within the selected type's budget. Architecture prefers 6–12 primary components; split above the figure budget.
- Remove low-value edges before adding routing controls. Put supporting detail in callouts, not extra edges.
- Keep the brief's focal element as its own coral node.
- Fan-in branches use separate elbows into a normal-height target (≤96px); never stretch a node to catch arrows.
- Subtitle and legend are absent unless they add information.

## Truth

- Preserve exact product names, identifiers, commands, protocols, API paths, and environments.
- Use only verified component identities. File proximity, naming, or an import does not prove a relationship.
- Keep external actors outside a boundary only when source evidence proves that boundary.
- Group only real ownership, trust, process, or deployment boundaries; boundaries never replace edges.

## Labels

Keep labels that carry protocol, action, direction, sync/async behavior, or a cross-boundary mechanism. Delete one only when both endpoints imply the relationship and no such meaning remains; report the deletion. Never drop meaning merely to pass a check.

## Placement

- **Architecture:** one left-to-right spine with short vertical branches.
- **Workflow:** lanes show responsibility or phase; columns show progress; retries and exceptions route outside the main corridor.
- **Sequence:** participants follow conversation order; messages follow time. First fragment message starts ≥40px below its header, or ≥48px for two lines.
- **Data flow:** stages show transformation or custody; rows separate parallel streams; label only non-obvious contracts, classification, consent, or boundary crossings.
- **Lifecycle:** phases sit on the main rail; event and terminal bands sit beneath later phases.