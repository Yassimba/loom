# Document templates

## architecture.md

```markdown
# Architecture: <system>

Requirements: <link>

## Placement
| Capability | Spot (module / package / deployable) | Deciding force |

## Module map
| Module | Responsibility | Subdomain | Owner team | Layer |

<graph from `tach show --mermaid -o -`>

## Integrations
| Edge | Knowledge shared | Strength | Distance | Volatility | Balanced? | Mechanism | Contract |

## Data ownership
| Store | Owner module | Readers and how they read |

## Key flows
Per main use case: the modules in order, the mechanism and contract at each edge, and where the
transaction starts and ends.

## Rule ledger
| # | Rule | Encoded by | Probe (file ← planted snippet) | Expected failure |
|---|---|---|---|---|
| R1 | ticketing uses billing only through its interface | `[[interfaces]] from = ["app.billing"]` | `ticketing/assign/__init__.py` ← `from app.billing._rules import price` | `is not part of the public interface for 'app.billing'` |

Each exception (`deprecated` edge, `tach-ignore`) gets a row with its ticket.

## Extraction
| Module | Integrators that bind it | Cost (low / medium / high) |

## Unresolved risks
Each accepted imbalance or open question, with the condition that reopens it.
```

## <module>/design.md

```markdown
# <module>

## Responsibility
## Encapsulated knowledge
What no other module knows.
## Interface
Exposed names, `contract` types and published events, with the invariants and error modes a
caller must know.
## Integrations
Per edge: direction, mechanism, strength, contract.
## Data
Owned stores, schema and role; read models of other modules' data.
## Change vectors
Plausible changes that touch only this module.
```
