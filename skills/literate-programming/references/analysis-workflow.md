# Codebase Analysis Workflow

How to analyze a codebase before writing the literate program.

## 1. Find Entry Points

The entry point is where execution begins. Identify it by ecosystem:

| Ecosystem     | Typical entry points                                      |
|---------------|-----------------------------------------------------------|
| Node / Bun    | `package.json` → `"main"`, `"bin"`, or `"scripts.start"` |
| Python        | `__main__.py`, `if __name__ == "__main__":`, `setup.py`   |
| Go            | `func main()` in `package main`                           |
| Rust          | `fn main()` in `src/main.rs`, or `lib.rs` for libraries  |
| C / C++       | `int main(...)` or framework-specific (`Qt`, `SDL`)       |
| Ruby          | `Gemfile`, `bin/` scripts, `config.ru` for Rails          |
| Java / Kotlin | `public static void main(String[] args)`                  |

For libraries without a binary entry point, start from the public API surface (exported modules, trait/interface definitions).

## 2. Trace Data Flow

Starting from the entry point, follow the data:

1. **What input does the program consume?** (CLI args, stdin, files, network, env vars)
2. **What transformations does it apply?** (parsing, validation, computation)
3. **What output does it produce?** (files, stdout, network responses, side effects)

Map this as a pipeline. Each stage becomes a candidate section in the literate program.

### Tips
- Follow function calls depth-first, but only go deep enough to understand the purpose—skip implementation details of well-known libraries.
- Look for data types that flow through multiple stages; these are the "spine" of the program.
- Note where errors are handled—this reveals what can go wrong and why.

## 3. Find Background / Parallel Processes

Many programs are not purely sequential. Look for:

- **Event loops**: `addEventListener`, `on('event')`, `select!`, `tokio::spawn`
- **Worker threads / processes**: `new Worker`, `threading.Thread`, `go func()`
- **Scheduled tasks**: `cron`, `setInterval`, `@Scheduled`
- **Message queues**: Kafka consumers, Redis pub/sub, channel receivers
- **Signal handlers**: `SIGTERM`, `SIGINT`, graceful shutdown logic

Document these as separate flows that interact with the main pipeline. They often deserve their own section.

## 4. Choose Diagram Types

Use Mermaid diagrams to illustrate structure and flow:

| What to show                     | Diagram type      |
|----------------------------------|-------------------|
| Module / package dependencies    | `graph TD`        |
| Request lifecycle                | `sequenceDiagram` |
| Data transformation pipeline     | `graph LR`        |
| State transitions                | `stateDiagram-v2` |
| Class / type hierarchy           | `classDiagram`    |
| Deployment / infrastructure      | `graph TD`        |
| Timeline of startup / shutdown   | `sequenceDiagram` |

### Guidelines
- One diagram per concept. Do not cram everything into one diagram.
- Label edges with the data type or action being performed.
- Keep diagrams to ~10-15 nodes max; split if larger.
- Place diagrams *before* the code they describe, so the reader has a mental model first.

## 5. Determine Psychological Order

Knuth's key insight: present code in the order that makes it easiest to *understand*, not the order the compiler needs.

### Strategies

**Top-down**: Start with the high-level architecture, then drill into components. Best for systems with clear layering (web servers, compilers, CLI tools).

**Data-centric**: Start with the core data types, then show operations on them. Best for data processing pipelines, parsers, and state machines.

**Narrative**: Follow the lifecycle of a request/event from arrival to completion. Best for servers, event-driven systems, and interactive applications.

**Problem-solution**: State a problem, show the solution. Repeat. Best for algorithmic code or code with many edge cases.

### Deciding the Order

1. Identify the single most important concept the reader must grasp.
2. Present that first, with minimal prerequisites.
3. For each subsequent section, ask: "What does the reader need to know before this makes sense?"
4. Arrange sections so each one builds on the previous.
5. Defer details (error handling, edge cases, config) until after the happy path is clear.

## 6. Handle Large Codebases

If the codebase is too large for a single `.lit.md`:

- **Focus**: Pick the most important subsystem (the "core loop") and write that.
- **Split**: Create multiple `.lit.md` files, each covering a subsystem. Link between them in prose.

### The Completeness Rule

Every file that has a root chunk (`file="path"`) **must have 100% of its content in chunks**. Tangle will overwrite that file, so any content not captured in chunks will be deleted.

If you don't want to manage a file in the literate program, simply don't create a root chunk for it. The tangler only writes files that have root chunks. You can still mention those files in prose.

### What to Include vs. Leave Alone

**Include as root chunks**: Core algorithms, non-obvious logic, architectural decisions, integration points. Every line of these files must be in chunks.

**Leave alone** (no root chunk): Lock files, build artifacts, vendored dependencies, generated code, config files you don't need to explain. Mention them in prose if relevant, but do not create root chunks for them.
