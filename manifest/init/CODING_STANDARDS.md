# Coding standards

Use these questions when writing or reviewing code.

## Types

- Is every public boundary and non-obvious value precisely typed?
- Does the code pass the strictest practical type-checker mode without broad escape types or casts?
- Does it use the language's current type syntax?
- Can a complete type replace strings, dictionaries, flags, or other loosely structured values?
- Do the types prevent invalid states instead of only documenting them?

## Structure

- Would a cohesive type make valid state and supported operations clearer, or would it only turn free functions into methods? Create the type only in the first case. Move functions into an existing type when they belong to its responsibility.
- Does each module or type have one clear responsibility?
- Does the design follow SOLID where those principles reduce coupling rather than add ceremony?
- Does the code follow the Law of Demeter? Ask an object to do the work instead of navigating through its internals.

## Indirection and size

Prefer the shortest direct path from caller to behavior.

- Does every interface, wrapper, factory, adapter, forwarding method, and service layer earn its place?
- Keep indirection only when it enforces policy, translates a real boundary, hides volatile details, or supports multiple implementations.
- Delete abstractions with one implementation unless they protect a real external boundary.
- Delete dead code, unused flexibility, speculative configuration, and dependencies replaced by the platform or standard library.
- Shrink repeated or procedural code when a shorter form stays clear.
- Count concepts as well as lines: fewer layers, types, names, and execution hops are preferable.
- Does the change reuse suitable code that already exists?

During review, report unnecessary complexity as:

- `delete:` remove code with no present responsibility.
- `indirection:` remove a layer or forwarding hop that adds no policy.
- `shrink:` preserve the behavior with fewer concepts or lines.
- `stdlib:` replace custom code with the standard library.
- `native:` replace code or a dependency with an existing platform or dependency feature.
- `yagni:` remove flexibility that no current requirement uses.

Format each finding as:

`path:L<line>: <tag> <what to cut>. <replacement>.`
