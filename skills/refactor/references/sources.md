---
description: Primary sources behind the refactor skill's design style, and the reading order for deeper design work.
---

# Sources

## Source Material

Use these established design ideas as practical tools, not doctrine:

- **deep modules and complexity pulled downward** from John Ousterhout's [A Philosophy of Software Design](https://stanford.edu/~ouster/cgi-bin/aposd.php) and his [discussion with Robert Martin](https://github.com/johnousterhout/aposd-vs-clean-code)
- **functional core, imperative shell** and value boundaries from Gary Bernhardt's [Boundaries](https://www.destroyallsoftware.com/talks/boundaries) and [Functional Core, Imperative Shell](https://www.destroyallsoftware.com/screencasts/catalog/functional-core-imperative-shell)
- **type-driven design and making illegal states unrepresentable** from Scott Wlaschin's [designing with types](https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/) and [Domain Modeling Made Functional](https://pragprog.com/titles/swdddf/domain-modeling-made-functional/)
- **parse, don't validate** from [Alexis King](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
- **Design by Contract** through preconditions, postconditions, assertions, and invariants from [Bertrand Meyer and Eiffel](https://www.eiffel.org/doc/solutions/Design_by_Contract_and_Assertions)
- **guard clauses** from Fowler's [Replace Nested Conditional with Guard Clauses](https://refactoring.com/catalog/replaceNestedConditionalWithGuardClauses.html)
- **YAGNI and evolutionary design** from Fowler's [YAGNI](https://martinfowler.com/bliki/Yagni.html)
- **semantic compression** and waiting for real examples before extracting reuse from [Casey Muratori](https://caseymuratori.com/blog_0015)
- **ports and adapters** from Cockburn's original [Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)
- **vertical slices** from Jimmy Bogard's [Vertical Slice Architecture](https://www.jimmybogard.com/vertical-slice-architecture/)
- **use-case-driven structure** from [Screaming Architecture](https://blog.cleancoder.com/uncle-bob/2011/09/30/Screaming-Architecture.html) and simple request workflows from Fowler's [Transaction Script](https://martinfowler.com/eaaCatalog/transactionScript.html)
- **locality of behavior** from [Carson Gross](https://htmx.org/essays/locality-of-behaviour/) and pragmatic complexity control from [The Grug Brained Developer](https://grugbrain.dev/)
- **encapsulation and behavior ownership** from Fowler's balanced treatment of [Tell, Don't Ask](https://martinfowler.com/bliki/TellDontAsk.html)

## Patterns Must Pay Rent — direct references

- Sandi Metz, [The Wrong Abstraction](https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction): "duplication is far cheaper than the wrong abstraction" and "prefer duplication over the wrong abstraction"
- Kent C. Dodds, [AHA Programming](https://kentcdodds.com/blog/aha-programming): Avoid Hasty Abstractions; wait until real use cases reveal the common shape
- Dan Abramov, [Goodbye, Clean Code](https://overreacted.io/goodbye-clean-code/): removing duplication can reduce the ability to change requirements and make code less maintainable
- John Ousterhout, [A Philosophy of Software Design](https://stanford.edu/~ouster/cgi-bin/aposd.php): deep modules hide substantial complexity behind small interfaces; shallow modules and classitis add interfaces without reducing cognitive load
- Jimmy Bogard, [Vertical Slice Architecture](https://www.jimmybogard.com/vertical-slice-architecture/): reject mandatory `Controller -> Service -> Repository` gates and let each use case adopt only the structure it needs
- Martin Fowler, [YAGNI](https://martinfowler.com/bliki/Yagni.html): speculative capabilities and abstractions impose build cost, delay cost, carry cost, and repair cost
- Martin Fowler, [Beck Design Rules](https://martinfowler.com/bliki/BeckDesignRules.html): after correctness, intention, and duplication, prefer the fewest possible classes and methods
- Casey Muratori, [Semantic Compression](https://caseymuratori.com/blog_0015): make code usable before making it reusable; extract only after real examples expose the shared semantics
- Carson Gross, [The Grug Brained Developer](https://grugbrain.dev/): do not factor too early; wait for narrow, stable cut points to emerge from working code
- Joel Spolsky, [Don't Let Architecture Astronauts Scare You](https://www.joelonsoftware.com/2001/04/21/dont-let-architecture-astronauts-scare-you/): abstraction can rise so far above the real user problem that it stops producing useful software
- Carson Gross, [Locality of Behaviour](https://htmx.org/essays/locality-of-behaviour/): an abstraction is harmful when readers must search distant files to discover what a local unit does

## Reading Order

When deeper design work is required, prefer this order:

1. [A Philosophy of Software Design](https://stanford.edu/~ouster/cgi-bin/aposd.php)
2. [Ousterhout versus Clean Code](https://github.com/johnousterhout/aposd-vs-clean-code)
3. [Parse, Don't Validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
4. [Making Illegal States Unrepresentable](https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/)
5. [Functional Core, Imperative Shell](https://www.destroyallsoftware.com/screencasts/catalog/functional-core-imperative-shell)
6. [YAGNI](https://martinfowler.com/bliki/Yagni.html)
7. [Semantic Compression](https://caseymuratori.com/blog_0015)
8. [Vertical Slice Architecture](https://www.jimmybogard.com/vertical-slice-architecture/)
9. [Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)
10. [Locality of Behaviour](https://htmx.org/essays/locality-of-behaviour/)
