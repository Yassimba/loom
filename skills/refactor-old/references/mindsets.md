---
description: The five mindsets that calibrate the deletion bias — pick the one that fits the target scope and name it in the report.
---

# Mindsets

Philosophy behind the deletion bias — why less is more. Routing lives in SKILL.md's mindset table; read the section for the mindset you picked.

## Simplicity vs Easy

**Simple** is objective: one concept, not intertwined with others. **Easy** is subjective: familiar, near at hand. Easy changes as you learn; simple is absolute.

"Complect" means to braid together. Complexity comes from complecting concepts that should stay separate — every coupling is a future debugging session. The easy path (the familiar pattern, the framework you know) often complects; the simple path may be unfamiliar, but it doesn't braid concerns.

When designing, ask: "Am I choosing this because it's simple, or because it's familiar?" Familiar feels productive; simple *is* productive over the lifetime of the code. Choose simple.

- [Simple Made Easy](https://www.infoq.com/presentations/Simple-Made-Easy/) — Rich Hickey's canonical talk on the distinction

## Design Is Taking Apart

Good design is not adding features — it's removing dependencies: separating concerns so cleanly that each piece can be understood, tested, and changed independently. When you see a complex system, the skill is seeing how to *pull it apart*: what concerns are mixed here, which responsibilities could be separate, where are we conflating concepts?

Small independent pieces compose freely, test trivially, and change safely. Inheritance complects; composition liberates. The anti-pattern is the god object — every helper method added to a class is a small step toward the kitchen sink.

Before adding a method, wrapper, or abstraction, ask: does this *separate* concerns or *combine* them? Could a function that takes data and returns data do it?

- [Out of the Tar Pit](https://curtclifton.net/papers/MosesleyMarks06a.pdf) — Moseley & Marks on essential vs accidental complexity
- [A Philosophy of Software Design](https://www.amazon.com/dp/173210221X) — John Ousterhout on deep vs shallow modules

## Data Over Abstractions

> "It is better to have 100 functions operate on one data structure than 10 functions on 10 data structures."

A `Map<String, Value>` can be processed by hundreds of existing functions; a `SettingsManager` class only by the methods you write for it. Every custom type adds a concept to understand, needs its own operations, and limits composition.

Model the information, not the behavior: what data exists, what are the relationships, what transformations are needed. Before creating a type, ask: could this be a dict with well-known keys, a tuple, a plain record? Save custom types for genuinely custom behavior. The power is in the combinations, not the custom constructs.

- [The Value of Values](https://www.infoq.com/presentations/Value-Values/) — Rich Hickey on data vs objects
- [Data-Oriented Design](https://www.dataorienteddesign.com/dodbook/) — Richard Fabian

## Happy Path First

If the happy path is 95% of runtime behavior, it should be roughly 95% of the code readers see. Top-level functions orchestrate a use case in language that reads almost like English; parsing, process plumbing, and protocol details live below deep, well-named boundaries — complexity pulled downward. A shallow helper that only renames a step is the opposite of depth: depth is a small interface hiding real mechanics.

Trust flows from boundaries. Parse external data once, into a type that carries the proof of validity, and let the interior receive trusted values — every interior re-check is then deletable, not just discouraged. Invalid conditions leave through guard clauses at the top; the valid path stays flat. Model mutually exclusive states directly so illegal combinations can't be constructed.

Complexity needs evidence. An edge case earns code only after a real runtime failure — a log, a reproduction, a user report — proves it exists, and then it gets the smallest fix at the boundary that owns it, not a general defense system. "Could", "might", and "what if" are not evidence. Patterns pay rent the same way: extract a port, repository, or value object only when it owns a real invariant, hides real complexity, or removes stable duplication — duplication is cheaper than the wrong abstraction.

- [A Philosophy of Software Design](https://stanford.edu/~ouster/cgi-bin/aposd.php) — Ousterhout on deep modules and pulling complexity downward
- [Parse, Don't Validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/) — Alexis King
- [Making Illegal States Unrepresentable](https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/) — Scott Wlaschin
- [Functional Core, Imperative Shell](https://www.destroyallsoftware.com/screencasts/catalog/functional-core-imperative-shell) — Gary Bernhardt
- [The Wrong Abstraction](https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction) — Sandi Metz
- [YAGNI](https://martinfowler.com/bliki/Yagni.html) — Fowler on the four costs of speculative capability
- [Semantic Compression](https://caseymuratori.com/blog_0015) — Muratori: make code usable before making it reusable
- [The Grug Brained Developer](https://grugbrain.dev/) — pragmatic complexity control

## PAGNI: Probably Are Gonna Need It

YAGNI's exceptions: things dramatically cheaper to build in than to retrofit. Data you can't get back (`created_at`/`updated_at`, audit logs, many-to-many when there's any hint of plural); infrastructure that touches everything later (API versioning, pagination, CI, logging); security fundamentals (session/password invalidation, a security@ address).

Before invoking PAGNI, all three must hold: retrofitting costs 10×+ (not 2×); it's a known pattern from experience (not speculation); adding it now is cheap (minutes or hours, not days). PAGNI is a short list learned from pain, not an escape hatch for over-engineering — when in doubt, YAGNI wins.

- [PAGNIs: Probably Are Gonna Need Its](https://simonwillison.net/2021/Jul/1/pagnis/) — Simon Willison
- [YAGNI Exceptions](https://lukeplant.me.uk/blog/posts/yagni-exceptions/) — Luke Plant
