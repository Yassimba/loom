# Balanced Coupling reference

Use this reference to judge an integration, not a module in isolation. Coupling makes a system work; the goal is balance, not universal decoupling.

## Modularity and complexity

A modular design makes both the location and outcome of a change predictable. Complexity appears when a change's consequences can only be understood after making it. Cognitive capacity is limited, so designs become complex when one change requires simultaneously reasoning about too many separated facts.

## Integration strength

Strength describes how much knowledge two modules share and therefore how likely change will cascade.

### Intrusive coupling

One module uses another's private interface or implementation details: internal storage, private objects, undocumented behavior, or incidental execution order. Treat all implementation knowledge as shared. This is strongest and often implicit.

### Functional coupling

Modules share functional requirements or duplicated rules. They must co-evolve when behavior changes even when no explicit dependency connects them. Common examples are duplicated validation, duplicated policy precedence, and callers that know a callee's workflow.

### Model coupling

Modules share a domain model. The coupling is explicit, but domain discoveries or representation changes propagate to every consumer. A large shared model creates more coupling than an integration-specific model.

### Contract coupling

A narrow, explicit integration contract hides implementation, functional rules, and internal domain models. Facades, DTOs, published languages, and deliberately small event schemas can create contract coupling. This is weakest, not automatically best: introducing a contract adds cost and distance.

Strength is also shaped by connascence: shared names and types are cheaper than shared meaning, algorithms, timing, values, or identity.

## Distance

Distance determines the cost of a cascading change. Judge it relative to the chosen abstraction level.

Technical distance grows from functions and files through modules, packages, deployables, and external systems. A cross-module relationship is high distance when modules are the level under review, even inside one repository.

Distance is socio-technical:

- separate owners and coordination increase it;
- independent release or deployment lifecycles increase it;
- synchronous runtime dependencies bind lifecycles more tightly;
- asynchronous integration increases independence and therefore distance;
- shared tests and mandatory co-deployment reduce effective distance.

Lower distance makes co-evolution cheaper but increases lifecycle coupling. Higher distance supports independent evolution only when integration strength is low enough.

## Volatility

Volatility is the probability the relevant knowledge will need to change.

- **Core subdomain:** differentiating behavior receiving continued investment; usually high essential volatility.
- **Supporting subdomain:** necessary custom work that does not differentiate the product; usually lower volatility.
- **Generic subdomain:** a solved capability available off the shelf; low functional volatility, but the chosen provider or technology may be replaceable.

Separate essential business volatility from implementation volatility. Commit frequency can be accidental volatility caused by poor design. Stability can be accidental involatility when fear or cost prevents desired change. Use product direction, domain evidence, and user confirmation—not Git activity alone—to classify volatility.

## The balance rule

At the extremes:

| | Low distance | High distance |
| --- | --- | --- |
| Low strength | Low cohesion: unrelated behavior is co-located | Loose coupling: modular |
| High strength | High cohesion: co-evolution is cheap | Tight coupling: complex |

`MODULARITY = STRENGTH XOR DISTANCE`

Add pragmatism:

`BALANCE = (STRENGTH XOR DISTANCE) OR NOT VOLATILITY`

Interpretation:

- High strength + low distance is balanced high cohesion.
- Low strength + high distance is balanced loose coupling.
- High strength + high distance is urgent only when volatile.
- Low strength + low distance suggests low cohesion and cognitive clutter.
- Any imbalance may be accepted when genuine volatility is low.

The formula is a reasoning aid, not a numerical score. A reportable issue still needs a plausible change vector, concrete cascading changes, evidence that distance makes them expensive, and a recommendation cheaper than doing nothing.

## DDD relationships

Bounded contexts control model coupling by deciding where one model applies. Context integration patterns adjust strength: shared kernels and partnerships accept stronger knowledge sharing; anti-corruption layers and open-host contracts reduce it across greater distance. Aggregates intentionally keep strongly coupled transactional behavior close.

Do not recommend these patterns by name unless the evidence justifies their cost. First decide whether strength or distance is the dimension that should change.

## Required links

When writing the report, link the first use of each concept:

- Balance rule and tight/loose coupling: https://coupling.dev/posts/core-concepts/balance/
- Complexity: https://coupling.dev/posts/core-concepts/complexity/
- Modularity: https://coupling.dev/posts/core-concepts/modularity/
- Coupling: https://coupling.dev/posts/core-concepts/coupling/
- Integration strength and its four levels: https://coupling.dev/posts/dimensions-of-coupling/integration-strength/
- Distance, lifecycle, runtime, and organizational distance: https://coupling.dev/posts/dimensions-of-coupling/distance/
- Volatility and subdomain classification: https://coupling.dev/posts/dimensions-of-coupling/volatility/
- Connascence: https://coupling.dev/posts/related-topics/connascence/
- Domain-driven design: https://coupling.dev/posts/related-topics/domain-driven-design/
