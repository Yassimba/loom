# Contract verification

Perform a read-only contract review. Read repository instructions. Do not edit files, publish reviews, or notify users unless separately requested.

## Scope

Use the requested PR, revision range, file, directory, or repository. Without an explicit scope, inspect the current task's committed branch changes plus staged, unstaged, and untracked changes. Infer the comparison base from repository context; ask only when no reliable baseline exists. When a workflow supplies captured commits, use that exact comparison even if the branch advances.

A diff review checks changes against every applicable contract. Report a pre-existing violation only when it directly affects changed behavior, an assumption the change relies on, or an introduced or modified contract. Explain that connection. Sharing a file, declaration, or call graph is insufficient. An audit without a diff covers its full selected scope.

## Procedure

1. Inspect the diff and surrounding implementation, or the complete selected code for an audit. For commit comparisons, use `git diff --find-renames <merge_base> <head_sha>` and `git show <merge_base>:<path>`. Inspect additions, modifications, deletions, and complete old/new contract bodies and metadata.
2. Discover local, enclosing-declaration, ancestor-directory, and relevant called-symbol contracts. `loom contracts affected --base <merge_base>` lists the candidate obligations for a diff, `--stale` those whose code moved without their text, `loom contracts propose --base <merge_base>` the changed declarations still carrying no contract, `loom contracts diff --base <merge_base>` every contract the change reworded or removed, and `loom contracts at PATH:LINE` those governing one location. Validate syntax with `loom contracts check` and discoverability with `loom contracts list`. If tooling is unavailable, inspect manually and disclose material limits.
3. Trace actual inputs, guards, errors, outputs, state changes, and side effects. Check contract validity, mutual consistency, and implementation compliance. Neither code nor prose is automatically correct, and weakening a contract in the same change does not excuse a regression.
4. For every introduced, changed, or explicitly targeted contract, inspect its declaration and consumers. `loom contracts related PATH:LINE --kind callers` lists them when a language server is on PATH. Trace imports, re-exports, aliases, wrappers, and member uses; confirm textual matches refer to the affected symbol.
5. At each inspected caller, discover its own applicable contracts. Verify both that the caller respects the callee's obligations and that changed callee guarantees preserve the caller's obligations.

Inspect at most 64 distinct callers or references per affected declaration. Deduplicate overlap and prioritize changed callers, high-risk behavior, and diverse usage. Record and disclose caller caps, uninspected scope, uncertain relationships, and tooling failures when they materially limit a conclusion. A failed tool is not evidence of a violation.

## Output

Use the invoking workflow's format. Otherwise report concise findings containing:

- contract ID and declaration or `CONTRACTS` path;
- exact source location;
- concrete evidence of the mismatch or contradiction;
- consequence; and
- whether the violation is pre-existing.

If no violations are found, state that for the inspected scope and include material verification limits. Never imply exhaustive verification when the caller cap or unavailable tooling prevented it.

Adapted from: <https://github.com/spolu/code-contracts/blob/6d1b1a945d1fe02a43bac567341b77ccfdf3ae33/skills/code-contracts/SKILL.md#on-demand-verification>
