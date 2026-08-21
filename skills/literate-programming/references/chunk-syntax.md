# Chunk Syntax Specification

## Fenced Code Block Attributes

Named chunks use standard fenced code blocks with attributes in curly braces:

````
```language {chunk="chunk-name"}
code here
```
````

Root chunks additionally specify the output file path:

````
```language {chunk="chunk-name" file="path/to/output.ext"}
code here
```
````

### Attribute Rules

- `chunk="..."` — Required. Names this block so it can be referenced elsewhere.
- `file="..."` — Optional. Marks this as a root chunk; the tangle tool writes the expanded result to this path (relative to `--output-dir`).
- The language identifier (e.g., `ts`, `python`, `go`) appears before the `{` and controls syntax highlighting.

## Chunk References

Inside a code block, reference another chunk with:

```
<<chunk-name>>
```

The reference must be on its own line. Leading whitespace before `<<` is preserved as indentation when the referenced chunk is expanded.

### Example

````
```ts {chunk="greet" file="src/hello.ts"}
<<imports>>

function greet(name: string): string {
  <<build-greeting>>
  return result;
}
```
````

````
```ts {chunk="imports"}
import { format } from "./utils";
```
````

````
```ts {chunk="build-greeting"}
const result = format(`Hello, ${name}!`);
```
````

Tangling `greet` produces:

```ts
import { format } from "./utils";

function greet(name: string): string {
  const result = format(`Hello, ${name}!`);
  return result;
}
```

Note how `build-greeting` inherits the 2-space indent from the reference site.

## Naming Conventions

- Use lowercase kebab-case: `parse-config`, `main-loop`, `db-connection`.
- Choose descriptive names that convey *purpose*, not file location.
- Keep names short but unambiguous within the document.

## Additive Chunks

Defining the same chunk name in multiple places **concatenates** the definitions in document order:

````
```python {chunk="imports"}
import os
```
````

*(later in the document)*

````
```python {chunk="imports"}
import sys
```
````

This produces:

```python
import os
import sys
```

Use additive chunks for progressive elaboration — introduce pieces of a concept as the narrative requires, rather than dumping everything at once.

## Root vs. Fragment Chunks

| Property        | Root chunk                        | Fragment chunk             |
|-----------------|-----------------------------------|----------------------------|
| Has `file=`     | Yes                               | No                         |
| Written to disk | Yes                               | No                         |
| Purpose         | Represents a complete source file | A reusable piece of logic  |
| Must resolve    | All `<<refs>>` must expand        | Only when pulled into root |

## Error Conditions

- **Undefined reference**: `<<name>>` where `name` is never defined → hard error.
- **Circular reference**: `A` → `B` → `A` → hard error.
- **Unused chunk**: Defined but never referenced by any root chunk → warning.
- **Unclosed fence**: A chunk opened but never closed → warning, treated as if closed at EOF.
