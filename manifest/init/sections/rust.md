## Rust

### Code Quality (Rust)

- Prefer narrow visibility by default because this workspace is generally its own consumer. However, do not add workarounds solely to avoid `pub`: make an item public when another workspace crate needs it and that produces the cleaner implementation.
- Rust imports should always go at the top of the file, never locally in functions.
- Try hard to avoid patterns that require `panic!`, `unreachable!`, `.unwrap()` or `.expect()`. Instead, try to encode those constraints in the type system. Don't be afraid to write code that's more verbose or requires largeish refactors if it enables you to avoid these unsafe calls.
- Prefer let chains (`if let` combined with `&&`) and let guards (`PAT if let ... =>`) over nested `if let` statements to reduce indentation and improve readability. At the end of a task, always check your work to see if you missed opportunities to use `let` chains or `let` guards.
- If you _have_ to suppress a Clippy lint, prefer to use `#[expect()]` over `[allow()]`, where possible. But if a lint is complaining about unused/dead code, it's usually best to just delete the unused code.
- Don't prefix tests with `test_`.
- Don't separate struct definitions from their `impl` blocks unless the `impl` is deliberately placed in a separate file, as for large structs.

### Commands (Rust)

- Run all tests (using `nextest` for faster execution, setting `CARGO_PROFILE_DEV_OPT_LEVEL=1 CARGO_PROFILE_DEV_DEBUG="line-tables-only"`

- Running Clippy

```sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
