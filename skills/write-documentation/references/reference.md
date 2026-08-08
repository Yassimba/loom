# Reference rules

Reference is a **dictionary**: authoritative technical description of the product and its operation, consulted by a competent reader during work. It supplies truth and certainty, not a lesson, task, or argument.

## Describe the machinery

Let the product determine the scope and arrangement of the material. Describe APIs, commands, classes, functions, configuration, behavior, constraints, and other machinery as succinctly and completely as the agreed scope requires.

Description can include how a component works or the correct conditions for using it. It must not turn into a procedure for accomplishing a reader goal.

Reference should be:

- **accurate:** every claim agrees with the product;
- **precise:** names, types, defaults, units, and conditions are exact;
- **complete within scope:** no scoped item or required field is omitted;
- **clear:** ambiguity is resolved rather than delegated to the reader;
- **austere:** unrelated context and interpretation do not obstruct lookup;
- **authoritative:** readers do not need to inspect the implementation for certainty.

Generated material can improve fidelity, but generation alone does not guarantee useful coverage, organization, or clarity.

## Mirror the product

Arrange the documentation so its logical and conceptual relationships correspond to those in the product. A reader navigating a module, command tree, configuration hierarchy, or API should recognize the same relationships in the reference.

Do not imitate implementation structure mechanically when it produces an unnatural reader-facing organization. Preserve the meaningful architecture of the machinery.

## Use standard patterns

Make lookup predictable. Use one heading pattern and one field order for peer entries. Include the applicable fields consistently, such as:

1. signature or syntax;
2. purpose or definition;
3. parameters, types, allowed values, and defaults;
4. return value or output;
5. effects and guarantees;
6. errors and warnings;
7. constraints and limitations;
8. a compact example, when useful.

Readers should know where a fact will appear before reading the entry. Prefer familiar terminology and repeated structures over stylistic variety.

## Use examples as illustration

A small example can reveal shape, context, or usage more efficiently than more description. Keep it subordinate to the entry: it illustrates a fact but does not become a walkthrough. Put task sequences in how-to guides.

## Keep the reference neutral

State facts without instruction, persuasion, speculation, or discursive interpretation.

Useful forms include:

- “The default configuration inherits…”
- “Available subcommands are…”
- “This option accepts…”
- “Use of X requires Y.”
- “Applying X when Y is enabled causes…”

Warnings are reference facts when they state a constraint, hazard, or invalid condition. Link to tutorials, how-to guides, and explanations when readers need learning, action, or understanding beyond the description.

## Completion

The reference is done when every item in the agreed scope has an entry and every peer entry follows the same structure with all applicable fields populated.

Source model: [Diátaxis — Reference](https://diataxis.fr/reference/).
