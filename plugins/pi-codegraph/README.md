# Pi CodeGraph prompt context

Runs `codegraph prompt-hook` before Pi processes a prompt and adds its output to
the model's context. CodeGraph decides whether the prompt is relevant and which
existing project index to query. Context messages are hidden in the TUI.

## Install

Use macOS, Linux, or WSL on Windows.

Install the CodeGraph CLI through Loom:

```sh
loom add --tool codegraph
```

Until this package is published, install from a Loom checkout:

```sh
pi install ./plugins/pi-codegraph
```

After the npm release, select **CodeGraph prompt context** in Loom, or run:

```sh
pi install npm:@yassimba/pi-codegraph
```

Run `codegraph init` in each project you want indexed, then `/reload` in Pi.
The extension requires `codegraph` on PATH and does not create indexes or install
an MCP server. You can also query the index with `codegraph explore` through Pi's
shell tool.

## Behavior

The extension passes the prompt and working directory as JSON on stdin, without
shell interpolation. It leaves prompt classification, index discovery, and
context limits to the installed CodeGraph version. Empty output, missing CLI,
command errors, and timeouts produce no context and let Pi continue. Commands
have a ten-second timeout and a one-MiB output buffer limit.

Set `CODEGRAPH_NO_PROMPT_HOOK=1` to disable context injection, or remove the
package with `pi remove` using the source shown by `pi list`, then reload Pi.
