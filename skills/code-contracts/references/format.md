# Code contract format

## Scope

A declaration contract lives in one supported documentation comment or docstring immediately associated with the declaration it governs. Multiple consecutive contract comments may attach to one declaration, but each comment contains exactly one contract.

A directory contract lives in a file named `CONTRACTS` and applies to that directory and all descendants. Ancestor and local directory contracts apply together.

Attached contract identity is the declaration's language-specific identity plus its contract ID. Directory contract identity is the repository-relative `CONTRACTS` path plus its ID. Keep attached IDs unique and stable within their declaration. Keep directory IDs unique and stable within their file and across its ancestor `CONTRACTS` chain.

## Syntax

```text
@cc [owner:alice;bob,notify:security,label:product;security] stable-contract-id
The observable requirement, written as Markdown-capable prose.
```

Metadata is optional. Well-known keys are:

- `owner`: GitHub users notified when an existing contract changes or is removed.
- `notify`: GitHub users notified when a violation is found. Owners are not implied.
- `label`: free-form classification.
- `test`: identifiers of tests that enforce this contract mechanically. `loom contracts check` fails when one is not defined anywhere in the repository.

Separate attributes with commas and multiple values within one attribute with semicolons. Metadata keys are extensible. The prose body must be non-empty.

```ebnf
contracts_file = { contract, NL } ;
contract       = directive, NL, prose ;
directive      = "@cc", SP, [ metadata, SP ], contract_id ;
metadata       = "[", attribute, { ",", attribute }, "]" ;
attribute      = key, ":", value ;
contract_id    = token ;
key            = token ;
value          = token ;
prose          = prose_line, { NL, prose_line } ;
```

A `token` is non-empty and contains no whitespace, commas, colons, or square brackets. Comment delimiters and decorations are removed before parsing.

## Tooling

The `loom` CLI validates and discovers contracts. It ships with the Loom setup itself, so nothing extra is installed.

```sh
loom contracts check                    # syntax and duplicate IDs, whole repository
loom contracts check path/to/file.rs
loom contracts at path/to/file.ts:42    # every contract governing a line
loom contracts list path/to/file.py     # contracts in a file or directory
loom contracts at path/to/file.py:42 --no-global   # skip inherited directory rules
loom contracts list some-contract-id    # one contract by ID
loom contracts affected --base main     # candidate obligations for a diff
loom contracts affected --stale         # code under a contract changed, its text did not
loom contracts diff --base main         # contracts added, reworded, or removed
loom contracts propose --base main      # changed declarations with no contract yet
loom contracts related path/to/file.py:12 --kind callers
```

`at` and `list` include directory contracts from ancestor `CONTRACTS` files; `--no-global` leaves them out. `check` reports malformed syntax, duplicate IDs, and `test` anchors nothing defines. `diff` pairs contracts across revisions by file, attached declaration, and ID, so a reworded or removed obligation exits 1 and names its owners; a renamed declaration shows as removed plus added. `propose` lists the innermost named declaration (`fn`, `def`, `class`, …) around each changed line when nothing is attached to it, with the rules that already govern it; it is the obligation sweep as a checklist and never writes prose. `affected`, `diff`, and `propose` compare the working tree against `--base`, so untracked files are invisible to them. Nothing rewrites files, and no command judges prose truth or implementation compliance — `affected` reports candidates for you to judge. Add `--json` to any command for one JSON object on stdout. Exit codes: 0 clean, 1 findings, 2 usage or IO error.

Supported: TypeScript (`.ts`, `.tsx`, `.mts`, `.cts`), Python (`.py`, `.pyi`), Rust, and `CONTRACTS` files. `related` additionally needs the language's server on PATH (`rust-analyzer`, `ty`, `typescript-language-server`); Loom never installs one. A language server analyses the real project, and `rust-analyzer` in particular may run build scripts and proc macros while doing so — the same code `cargo check` would execute. For unsupported languages, use directory contracts or inspect declaration comments manually and disclose the tooling limit.

Source specification: <https://github.com/spolu/code-contracts/blob/6d1b1a945d1fe02a43bac567341b77ccfdf3ae33/README.md#specification-and-grammar>
