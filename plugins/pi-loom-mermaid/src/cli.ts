#!/usr/bin/env -S node --experimental-strip-types

import { transformMermaidForDocument } from "./index.ts";

let markdown = "";
for await (const chunk of process.stdin) markdown += chunk;

const width = Number.parseInt(process.env.LOOM_MERMAID_WIDTH ?? "100", 10);
process.stdout.write(transformMermaidForDocument(markdown, Number.isFinite(width) ? width : 100));
