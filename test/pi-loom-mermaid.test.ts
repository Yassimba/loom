import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { scanFences } from "../plugins/pi-loom-mermaid/src/fences.ts";
import piLovelyMermaid, { transformMermaidMarkdown } from "../plugins/pi-loom-mermaid/src/index.ts";
import { Graph } from "../plugins/pi-loom-mermaid/src/loom-mermaid/graph.ts";
import { render, toAnsi } from "../plugins/pi-loom-mermaid/src/loom-mermaid/index.ts";
import { LIMITS } from "../plugins/pi-loom-mermaid/src/loom-mermaid/labels.ts";
import { computeRanks } from "../plugins/pi-loom-mermaid/src/loom-mermaid/layout-rank.ts";
import { bicliqueKeys } from "../plugins/pi-loom-mermaid/src/loom-mermaid/layout-tracks.ts";
import { diagramFor } from "../plugins/pi-loom-mermaid/src/loom-mermaid/registry.ts";

test("adds Mermaid guidance to the existing system prompt on each agent start", () => {
  let handler: ((event: { systemPrompt: string }) => { systemPrompt: string }) | undefined;
  piLovelyMermaid({
    registerMarkdownTransformer() {},
    on(event: string, callback: typeof handler) {
      if (event === "before_agent_start") handler = callback;
    },
  } as unknown as ExtensionAPI);

  assert.ok(handler);
  for (const base of ["Original instructions", "Updated instructions"]) {
    const { systemPrompt } = handler({ systemPrompt: base });
    assert.ok(systemPrompt.startsWith(`${base}\n\n`));
    assert.match(systemPrompt, /Always use Mermaid when it is easier to read than prose/);
    assert.match(systemPrompt, /architecture for deployed services/);
    assert.match(systemPrompt, /flowchart for dependencies\/decisions/);
    assert.match(systemPrompt, /:::red \(removed\), :::green \(added\), or :::orange \(changed\)/);
    assert.match(
      systemPrompt,
      /outside its label brackets \(A\[Added\]:::green, never A\[Added :::green\]\)/,
    );
    assert.match(
      systemPrompt,
      /default colors are automatic—do not add classDef for these diff markers/,
    );
    assert.match(systemPrompt, /prefer colored outlines/);
  }
});

test("node styles color only the outline", () => {
  for (const node of ["A[Help]", "A(Help)", "A{Help}"]) {
    const art = render(
      `flowchart TD\n ${node}:::custom\n classDef custom fill:#ecfdf5,stroke:#4f8560,color:#000`,
    );
    assert.ok(art);
    const output = toAnsi(art).join("\n");
    assert.match(output, /38;2;79;133;96/, "outline retains its stroke");
    assert.doesNotMatch(output, /48;2/, "fill does not become a background");
    assert.doesNotMatch(output, /38;2;0;0;0/, "text keeps the terminal theme");
  }
});

test("renders Mermaid fences nested inside list items", () => {
  const markdown = `2. CLI help loads less code

    \`\`\`mermaid
    flowchart LR
      M[Extension manifests] --> H[Root help]:::orange
    \`\`\`
`;

  const output = transformMermaidMarkdown(markdown, {
    messageType: "assistant",
    availableWidth: 120,
  });

  assert.match(output, /Extension manifests/);
  assert.match(output, /Root help/);
  assert.doesNotMatch(output, /```mermaid/);
});

test("renders diff classes as dim colored borders without colored backgrounds", () => {
  const markdown = `\`\`\`mermaid
flowchart LR
  A[Removed]:::red --> B[Changed]:::orange --> C[Added]:::green
\`\`\``;

  const output = transformMermaidMarkdown(markdown, {
    messageType: "assistant",
    availableWidth: 200,
  });

  assert.match(output, /┌|╭|╔/);
  assert.ok(output.includes("\u001b[2;38;2;159;85;85m"));
  assert.ok(output.includes("\u001b[2;38;2;154;116;56m"));
  assert.ok(output.includes("\u001b[2;38;2;79;133;96m"));
  assert.doesNotMatch(output, /48;2/);
  assert.doesNotMatch(output, /classDef/);
});

test("vendored renderer handles every advertised diagram kind", () => {
  const diagrams = [
    "architecture-beta\n service api(server)[API]\n service db(database)[Database]\n api:R --> L:db",
    "flowchart LR\n A --> B",
    "stateDiagram-v2\n A --> B",
    "classDiagram\n class A",
    "erDiagram\n A ||--o{ B : has",
    "sequenceDiagram\n A->>B: hello",
    'pie\n "A" : 1',
    "mindmap\n root((A))\n  B",
    "timeline\n 2026 : Shipped",
    "gitGraph\n commit",
  ];

  for (const source of diagrams) {
    assert.ok(render(source), source.split("\n", 1)[0]);
    const live = transformMermaidMarkdown(`\`\`\`mermaid\n${source}\n`, {
      messageType: "assistant",
      availableWidth: 120,
      isStreaming: true,
    });
    assert.doesNotMatch(live, /```mermaid|Drawing Mermaid/, source);
  }
});

test("renders architecture groups, services, junctions, and arrows", () => {
  const drawn = render(`architecture-beta
  group cloud(cloud)[Cloud]
  group data(database)[Data] in cloud
  service api(server)[API] in cloud
  service db(database)[Database] in data
  service cdn(logos:aws-cloudfront)[CDN] in cloud
  junction route in cloud
  api:R <--> L:route
  route:R --> L:db{group}`);

  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  for (const label of ["☁ Cloud", "Data", "▣ API", "◉ Database", "[aws-cloudfront] CDN", "•"])
    assert.ok(text.includes(label), label);
  assert.match(text, /◄/);
  assert.match(text, /▶/);
  assert.deepEqual(drawn.warnings, []);
});

test("architecture accepts icon-less declarations and ignores align directives", () => {
  const drawn = render(`architecture-beta
group one(cloud)[One]
group two[Two]
service server[Server] in one
service subnet in two
server{group}:B --> T:subnet{group}
align row server subnet`);
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  for (const label of ["Two", "Server", "subnet"]) assert.ok(text.includes(label), label);
  assert.deepEqual(drawn.warnings, []);
});

test("architecture ports pick the flow axis and cross-axis ports do not detour", () => {
  const vertical = render(`architecture-beta
service a(server)[A]
service b(server)[B]
service c(server)[C]
a:B --> T:b
b:B --> T:c`);
  assert.ok(vertical);
  assert.match(vertical.plain.join("\n"), /A[\s\S]*B[\s\S]*C/);
  assert.ok(vertical.plain.length > vertical.width, "stacked, not side by side");

  const mixed = render(`architecture-beta
group g(cloud)[G]
service x(server)[X] in g
service y(server)[Y] in g
service z(server)[Z]
x:R --> L:z
y:B --> T:z`);
  assert.ok(mixed);
  const rows = mixed.plain;
  const frameBottom = rows.findIndex((r) => r.startsWith("└"));
  assert.equal(
    rows
      .slice(frameBottom + 1)
      .join("")
      .trim(),
    "",
    "nothing routed below the frame",
  );
  assert.equal(
    rows.filter((r) => r.includes("▶")).length,
    1,
    "both edges merge into Z from the left",
  );
});

test("architecture edges honor all four requested target ports", () => {
  for (const [ports, arrow] of [
    ["R --> L", "▶"],
    ["L --> R", "◄"],
    ["B --> T", "▼"],
    ["T --> B", "▲"],
  ]) {
    const drawn = render(`architecture-beta
  service a(server)[A]
  service b(server)[B]
  a:${ports}:b`);
    assert.ok(drawn);
    assert.match(drawn.plain.join("\n"), new RegExp(arrow), ports);
  }
});

test("LR overflow retries TD without changing the source or later wide renders", () => {
  for (const header of ["flowchart LR", "graph lr"]) {
    const source = `${header}\nA[Alpha] --> B[Beta] --> C[Gamma] --> D[Delta]`;
    const original = source;
    const wide = render(source);
    const down = render(source.replace(header, "flowchart TD"));
    assert.ok(wide && down);
    assert.deepEqual(render(source, { maxWidth: wide.width }), wide, "fitting LR stays LR");
    assert.deepEqual(render(source, { maxWidth: 20 }), down, "overflow retries TD");
    assert.equal(source, original);
    assert.deepEqual(render(source), wide, "unbounded render restores the parsed LR direction");
    assert.deepEqual(render(source, { maxWidth: wide.width }), wide, "widening restores LR");

    const markdown = `\`\`\`mermaid\n${source}\n\`\`\``;
    const transform = (availableWidth: number) =>
      transformMermaidMarkdown(markdown, {
        messageType: "assistant",
        availableWidth,
      });
    const cachedWide = transform(100);
    assert.match(transform(20), /▼/);
    assert.equal(transform(100), cachedWide, "width-keyed cache restores LR");
    assert.match(cachedWide, /▶/);
  }
});

test("LR keeps tighter horizontal labels before trying TD, then tries TD before collapsing", () => {
  const horizontal = "flowchart LR\nA[One two three four five six] --> B[Seven eight nine ten]";
  const diagram = diagramFor(horizontal);
  assert.ok(diagram);
  const tight = diagram.render(horizontal, LIMITS[1])?.canvas.toLines();
  const loose = render(horizontal);
  assert.ok(tight && loose && tight.width < loose.width);
  assert.deepEqual(render(horizontal, { maxWidth: tight.width })?.plain, tight.plain);

  const grouped =
    "flowchart LR\nsubgraph Work\nA[Alpha] --> B[Beta] --> C[Gamma] --> D[Delta]\nend";
  const fitted = render(grouped, { maxWidth: 20 });
  assert.deepEqual(fitted, render(grouped.replace("LR", "TD")));
  assert.ok(fitted);
  assert.match(fitted.plain.join("\n"), /Alpha/);
  assert.deepEqual(fitted.warnings, [], "full TD wins before the collapsed overview");
});

test("LR and TD overflow retain the existing collapsed and raw-source fallbacks", () => {
  const source = "flowchart LR\nsubgraph Work\nA[Alpha] --> B[Beta] --> C[Gamma] --> D[Delta]\nend";
  const collapsed = diagramFor(source)?.render(source, LIMITS[LIMITS.length - 1]);
  const narrow = render(source, { maxWidth: 1 });
  assert.ok(collapsed && narrow);
  assert.deepEqual(narrow.plain, collapsed.canvas.toLines().plain);
  assert.match(narrow.warnings.join("\n"), /subgraphs drawn collapsed/);
  assert.ok(narrow.width > 1, "caller still decides how to handle an oversized result");
  const markdown = `\`\`\`mermaid\n${source}\n\`\`\``;
  assert.equal(
    transformMermaidMarkdown(markdown, {
      messageType: "assistant",
      availableWidth: 1,
    }),
    markdown,
  );
});

test("TD retry refuses member edges across group boundaries instead of merging them", () => {
  const groups = `flowchart LR
subgraph S
A --> B --> C --> D
A --> X
end
subgraph T
E --> F --> G --> H
E --> Y
end`;
  const source = `${groups}\nD --> H\nX --> Y`;
  const diagram = diagramFor(source);
  const wide = render(source);
  const down = render(source.replace("LR", "TD"));
  assert.ok(diagram && wide && down);
  assert.ok(wide.width > down.width);
  const collapsed = diagram.render(source, LIMITS[LIMITS.length - 1]);
  const narrow = render(source, { maxWidth: down.width });
  assert.ok(collapsed && narrow);
  assert.deepEqual(narrow.plain, collapsed.canvas.toLines().plain);
  assert.match(narrow.warnings.join("\n"), /subgraphs drawn collapsed/);
  assert.deepEqual(render(source), wide, "wide LR keeps the original member-level edges");

  for (const unsafe of [
    source,
    `${groups}\nOutside --> A`,
    "flowchart LR\nsubgraph Parent\nA\nsubgraph Child\nB\nend\nend\nA --> B",
    "flowchart LR\nsubgraph S\nA --> T\nend\nsubgraph T\nB\nend",
  ]) {
    assert.equal(diagram.renderDown?.(unsafe, LIMITS[0]), null, unsafe);
  }
  for (const safe of [groups, `${groups}\nS --> T`]) {
    const expected = render(safe.replace("LR", "TD"));
    assert.ok(expected);
    assert.deepEqual(
      diagram.renderDown?.(safe, LIMITS[0])?.canvas.toLines().plain,
      expected.plain,
      "independent groups and group-level arrows remain eligible",
    );
  }
});

test("TD retry leaves explicit group directions and other diagram directions alone", () => {
  for (const source of [
    "flowchart LR\nsubgraph Work\ndirection LR\nA --> B --> C --> D\nend",
    "flowchart RL\nA --> B --> C --> D",
    "stateDiagram-v2\ndirection LR\nA --> B\nB --> C\nC --> D",
    "classDiagram\ndirection LR\nA --> B\nB --> C\nC --> D",
    "erDiagram\ndirection LR\nA ||--o{ B : has\nB ||--o{ C : has",
  ]) {
    const diagram = diagramFor(source);
    assert.ok(diagram);
    let existing = diagram.render(source, LIMITS[0])?.canvas.toLines();
    for (const limits of LIMITS) {
      existing = diagram.render(source, limits)?.canvas.toLines();
      assert.ok(existing);
      if (existing.width <= 20) break;
    }
    assert.deepEqual(render(source, { maxWidth: 20 })?.plain, existing?.plain);
  }
  const source = 'flowchart LR\n%% direction LR\nA["direction LR"] --> B --> C --> D';
  const down = render(source.replace("flowchart LR", "flowchart TD"));
  assert.ok(down);
  assert.deepEqual(
    render(source, { maxWidth: down.width }),
    down,
    "labels/comments are not direction declarations",
  );
});

test("a diagram wider than the space is laid out again with tighter labels", () => {
  const source = readFileSync(
    new URL("./fixtures/mermaid/skip-labelled.mmd", import.meta.url),
    "utf8",
  );
  const loose = render(source);
  const fitted = render(source, { maxWidth: 45 });
  assert.ok(loose && fitted);
  assert.ok(loose.width > 45);
  assert.ok(fitted.width <= 45);
  assert.equal(render(source)?.width, loose.width, "limits are restored after a retry");

  const output = transformMermaidMarkdown(`\`\`\`mermaid\n${source}\`\`\``, {
    messageType: "assistant",
    availableWidth: 45,
  });
  assert.doesNotMatch(output, /```mermaid/, "fits instead of falling back to source");
});

test("two edges passing through one cell cross as a hop, junctions draw as tees", () => {
  // dense.mmd concentrates into one trunk per target: no crossing left to
  // hop. subgraphs-lr still has a lane crossing a bus.
  const dense = render(
    readFileSync(new URL("./fixtures/mermaid/dense.mmd", import.meta.url), "utf8"),
  );
  assert.ok(dense);
  assert.doesNotMatch(
    dense.plain.join("\n"),
    /●/,
    "a fork or join is a tee, never a dot: every merged run is a real edge set",
  );
  const grouped = render(
    readFileSync(new URL("./fixtures/mermaid/subgraphs-lr.mmd", import.meta.url), "utf8"),
  );
  assert.ok(grouped);
  assert.match(grouped.plain.join("\n"), /╫/, "a lane crossed by another edge's bus is a hop");
});

test("a left-to-right lane takes the side its endpoints can reach without piercing a box", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/lane-stacked.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  // D sits under C; its return to A runs below the diagram, not up through C.
  assert.doesNotMatch(text, /┴─┐\n│ C │/, "no line enters C's top");
  assert.match(text, /└───┘\n\s+▲/, "the return arrives under A");
});

test("a lane arriving under a box keeps off the column its departing lanes use", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/lane-shared-port.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  // The dotted skip into Run lands beside the solid one leaving it.
  assert.match(text, /▲ │.*\n.*╌┘ │/, "dotted arrival and solid departure on separate columns");
});

test("two differently named relations into one entity both keep their name", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/er-cardinalities.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  for (const verb of ["contains", "ordered in", "billed by", "places", "uses", "stocks"]) {
    assert.match(text, new RegExp(verb), `${verb} is drawn`);
  }
});

test("streaming advances on completed statements and holds while a label arrives", () => {
  const streaming = { messageType: "assistant" as const, availableWidth: 80, isStreaming: true };
  const prefix = "```mermaid\nflowchart TD\n  A[First] --> B[Second]\n";
  const first = transformMermaidMarkdown(prefix, streaming);
  assert.doesNotMatch(first, /```mermaid/);
  assert.match(first, /First/);
  for (const tail of [
    " B --",
    " B --> C[Thi",
    ' B --> C["Third;\nnode',
    " B --> C[Third\nnode",
    ' %% comment with " and ;',
  ]) {
    assert.equal(transformMermaidMarkdown(prefix + tail, streaming), first);
  }
  const next = `${prefix} B --> C[Third]\n`;
  assert.match(transformMermaidMarkdown(next, streaming), /Third/);
  const closed = `${next}\`\`\`\n`;
  assert.equal(
    transformMermaidMarkdown(next, streaming),
    transformMermaidMarkdown(closed, streaming),
  );
  const t = performance.now();
  for (let i = 0; i < 200; i++) transformMermaidMarkdown(`${prefix} B --> C[Thi`, streaming);
  assert.ok(performance.now() - t < 100, "partial-token updates reuse the completed prefix");
});

test("streaming supports semicolons and respects the opening fence length and character", () => {
  const streaming = { messageType: "assistant" as const, availableWidth: 80, isStreaming: true };
  for (const fence of ["```", "~~~~", "````"]) {
    const prefix = `${fence}mermaid\nflowchart TD; A[First] --> B[Second];`;
    const first = transformMermaidMarkdown(prefix, streaming);
    assert.match(first, /First/);
    assert.doesNotMatch(first, /mermaid/);
    assert.equal(transformMermaidMarkdown(`${prefix} B --> C[Incomplete`, streaming), first);
    assert.equal(transformMermaidMarkdown(`${prefix}\n~~`, streaming), first);
    assert.equal(transformMermaidMarkdown(`${prefix}\n\`\``, streaming), first);
  }
});

test("streaming has no diagram leakage across blocks and keeps final fallbacks", () => {
  const streaming = { messageType: "assistant" as const, availableWidth: 80, isStreaming: true };
  const first = "```mermaid\nflowchart TD\n A[First]\n```\n\n";
  const output = transformMermaidMarkdown(
    `${first}\`\`\`mermaid\nflowchart TD\n B[Unfinished`,
    streaming,
  );
  assert.equal(output.match(/First/g)?.length, 1);
  assert.doesNotMatch(output, /Unfinished/);
  for (const markdown of ["```mermaid\nunsupported\n A --> B", "```js\nconst x = 1"]) {
    assert.equal(transformMermaidMarkdown(markdown, streaming), markdown);
  }
  const invalid = "```mermaid\nflowchart TD\n```";
  assert.equal(transformMermaidMarkdown(invalid, { ...streaming, isStreaming: false }), invalid);
  assert.equal(
    transformMermaidMarkdown(first, { ...streaming, messageType: "assistant-thinking" }),
    first,
  );
});

test("explicit node styles render only the stroke color", () => {
  const markdown =
    "```mermaid\nflowchart TD\n A[Help text]:::custom\n classDef custom fill:#ecfdf5,stroke:#4f8560,color:#000\n```";
  const output = transformMermaidMarkdown(markdown, {
    messageType: "assistant",
    availableWidth: 80,
  });
  assert.doesNotMatch(output, /48;2/, "author fill is ignored");
  assert.match(output, /38;2;79;133;96/, "author stroke is preserved");
  assert.doesNotMatch(output, /38;2;0;0;0/, "author text color is ignored");
});

test("a fan-out forks before another edge joins its row, and no head marks the tee", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/fork-before-join.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  // B's fork to Z (`┬`) comes before A's join (`┴`), so the joined edge
  // never reads as forking too. Runs merge only where merging is correct,
  // so the tee needs no head to disambiguate it.
  assert.match(text, /│ B ├─┬─┴─▶│ Y │/);
  assert.doesNotMatch(text, /[▼▲] {2}┌───┐\n│ B/, "no head feeding the tee");
});

test("rank assignment preserves cycles, parallel edges, and disconnected nodes", () => {
  for (const { edges, ranks } of [
    { edges: [], ranks: [] },
    { edges: [], ranks: [0, 0, 0] },
    {
      edges: [
        [0, 0],
        [0, 1],
        [1, 2],
      ],
      ranks: [0, 1, 2, 0],
    },
    {
      edges: [
        [0, 1],
        [1, 2],
        [2, 0],
      ],
      ranks: [0, 1, 2],
    },
    {
      edges: [
        [0, 1],
        [1, 0],
      ],
      ranks: [0, 1],
    },
    {
      edges: [
        [0, 1],
        [1, 3],
        [0, 2],
        [2, 4],
        [4, 3],
        [1, 3],
        [1, 3],
      ],
      ranks: [0, 2, 1, 3, 2],
    },
  ]) {
    const graph = new Graph();
    for (let i = 0; i < ranks.length; i++) graph.nodeIndex(String(i), null, "rect");
    for (const [from, to] of edges) {
      graph.pushEdge({ from, to, label: null, headFrom: "none", headTo: "arrow", line: "solid" });
    }
    assert.deepEqual(computeRanks(graph), ranks);
  }
});

test("overlapping bicliques never reassign an edge to a later bundle", () => {
  const graph = new Graph();
  const ranks = [0, 0, 0, 0, 1, 1, 1];
  for (let i = 0; i < ranks.length; i++) graph.nodeIndex(String(i), null, "rect");
  for (const [from, to] of [
    [0, 4],
    [0, 5],
    [1, 4],
    [1, 5],
    [1, 6],
    [2, 5],
    [2, 6],
    [3, 5],
    [3, 6],
  ]) {
    graph.pushEdge({ from, to, label: null, headFrom: "none", headTo: "arrow", line: "solid" });
  }
  assert.deepEqual(
    [...bicliqueKeys(graph, ranks)],
    [
      [0, "0,1>4,5"],
      [1, "0,1>4,5"],
      [2, "0,1>4,5"],
      [3, "0,1>4,5"],
      [5, "2,3>5,6"],
      [6, "2,3>5,6"],
      [7, "2,3>5,6"],
      [8, "2,3>5,6"],
    ],
  );
});

test("two sources sharing two targets ride one trunk, a private dotted edge takes its own track and head", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/biclique-private.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  assert.match(text, /│ RDF Extension +├─+┬─┼─+▶│/, "dotted fork, then the trunk join");
  // Two private arrivals into one target must share the approach or
  // cross (PrivateFanIn.lean): the dotted arm drops to Turbine's row
  // and joins its fan there, no hop.
  assert.doesNotMatch(text, /╫/, "no hop");
  assert.match(
    text,
    /\n +optional ╎ │ /,
    "the label sits beside its own dotted leg, not on a shared row",
  );
  assert.match(text, /│ Turbine Engine ├─┴─┴─+▶│ +Schema Inspector/, "one head for both");
});

test("LR class diagram: skips order freely, colliding labels move a row, no bus crossing", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/class-lr-skips.mmd", import.meta.url), "utf8"),
    { maxWidth: 200 },
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  assert.doesNotMatch(text, /╫/, "no bus crossing");
  for (const label of ["argument_", "name from", "spec from", "arguments"]) {
    assert.ok(text.includes(label), `label ${label} survives`);
  }
  assert.doesNotMatch(text, /arname/, "labels never overwrite each other");
});

test("class diagram: hierarchy ranks first, a sink sits at the edge its own arrows use", () => {
  const drawn = render(
    "classDiagram\n  direction LR\n  class Repo {\n    findAllByTenantAndStatus(tenant, status, page, sort) List~Row~\n  }\n  class Svc\n  class Row\n  Svc o-- Repo\n  Svc o-- Row\n",
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  // Row is a sink: it keeps the column's left edge, so the wide Repo
  // beside it does not stretch what arrives.
  assert.match(text, /└─{1,4}│ Row │/, "a sink is reached in a short run");
});

test("a lane the drawing already implies runs outside the ones that carry information", () => {
  // A -> D repeats A -> B -> C -> D, so it is the redundant one; E -> D is
  // the only way E reaches D. The redundant lane takes the outer track.
  const drawn = render("flowchart LR\n  A --> B --> C --> D\n  A --> D\n  E --> D\n  E --> F\n");
  assert.ok(drawn);
  const rows = drawn.plain;
  const boxes = rows.findIndex((l) => l.includes("│ A ├"));
  const above = rows.slice(0, boxes).filter((l) => l.includes("│")).length;
  assert.ok(above >= 2, "the redundant A -> D reaches over the top");
  assert.ok(
    rows.slice(boxes + 1).some((l) => l.includes("└─")),
    "the informative E -> D keeps the near track below",
  );
});

test("labels out of one source sit on their own arms, not stacked at the fork", () => {
  const drawn = render("flowchart LR\n  A -->|alpha| B\n  A -->|beta| C\n  A -->|gamma| D\n");
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  for (const [label, box] of [
    ["alpha", "B"],
    ["beta", "C"],
    ["gamma", "D"],
  ]) {
    const row = drawn.plain.findIndex((l) => l.includes(`│ ${box} │`));
    assert.ok(drawn.plain[row - 1].includes(label), `${label} labels the arm into ${box}`);
  }
  assert.doesNotMatch(text, /├─(alpha|beta|gamma)/, "no label written over the line");
});

test("a label never lands on its own trunk when the source sits against the bus", () => {
  const drawn = render(
    "flowchart LR\n  A -->|alpha| B\n  A -->|beta| C\n  B -->|gamma| D\n  C -->|delta| D\n",
  );
  assert.ok(drawn);
  const rows = drawn.plain;
  const trunk = rows.findIndex((l) => l.includes("delta"));
  const col = rows[trunk].indexOf("delta");
  // The vertical the label sits beside is still drawn on the row it shares.
  assert.ok(rows[trunk].slice(0, col).includes("│"), "the trunk survives left of the label");
  assert.doesNotMatch(rows[trunk], /│delta|delta│/, "the label keeps a column clear of the line");
});

test("a label clears every trunk in its band, not only its own", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/biclique-private.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const rows = drawn.plain;
  const row = rows.findIndex((l) => l.includes("optional"));
  const at = rows[row].indexOf("optional");
  // Both the dotted track and the solid trunk stay drawn on the label's row.
  assert.match(rows[row].slice(at), /^optional ╎ │/, "both trunks survive the label row");
});

test("a lane leg climbs past the boxes stacked under its target, not through them", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/lane-past-stack.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const rows = drawn.plain;
  // layout.ts -> layout-geom.ts returns along the bottom lane and climbs
  // to geom, whose column also holds paint.ts.
  const paint = rows.findIndex((l) => l.includes("│ paint.ts ├"));
  const box = rows[paint].indexOf("│ paint.ts ├");
  const leg = rows[paint].slice(0, box);
  assert.match(leg, /[│╫]/, "the leg is drawn beside the box, not swallowed by it");
});

test("the path of boxes runs straight and the return keeps to the side", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/loop-branch.mmd", import.meta.url), "utf8"),
  );
  assert.ok(drawn);
  const rows = drawn.plain;
  const col = (needle: string): number => {
    const row = rows.find((l) => l.includes(needle)) as string;
    return row.indexOf(needle) + needle.length / 2;
  };
  // start, check, process, more? and finish share one column; the branch
  // to reject splits off it and the return climbs outside the boxes.
  const spine = ["│ start │", "║ check ║", "│ process │", "╢ more? ║", "│ finish │"].map(col);
  assert.ok(Math.max(...spine) - Math.min(...spine) <= 1, `spine drifts: ${spine.join()}`);
  assert.ok(col("│ reject │") > Math.max(...spine), "reject splits to the side");
  assert.ok(
    rows.some((l) => l.includes("yes")),
    "the return keeps its label",
  );
});

test("a diff marker written inside a label or an edge is taken as the class it meant", () => {
  for (const source of [
    'flowchart TD\n  A["removed :::red"]:::red --> B[kept :::green]\n  B --> C[plain]\n',
    "flowchart LR\n  A -->|sent :::red| B\n",
    "classDiagram\n  A <|-- B : owns :::green\n",
    'classDiagram\n  class C["boxed :::red"]\n',
    "stateDiagram-v2\n  A --> B : go :::red\n",
  ]) {
    const drawn = render(source);
    assert.ok(drawn, source);
    assert.doesNotMatch(drawn.plain.join("\n"), /:::/, `marker reaches the canvas: ${source}`);
    assert.ok(
      drawn.warnings.some((w) => w.includes("goes after the bracket")),
      `no warning for: ${source}`,
    );
  }
  // The rest of the text survives, and the stroke still takes the colour.
  const drawn = render('flowchart TD\n  A["removed :::red"] --> B[kept]\n');
  assert.ok(drawn);
  assert.match(drawn.plain.join("\n"), /│ removed │/);
});

test("leaves hanging off one side of a trunk share a left edge", () => {
  const src = readFileSync("test/fixtures/mermaid/side-lane.mmd", "utf8");
  const art = render(src);
  assert.ok(art);
  const left = (label: string): number =>
    art.plain.find((l) => l.includes(label))?.indexOf("│") ?? -1;
  assert.equal(left("return null"), left("MermaidArt"));
});

test("a trunk of close widths is padded to one width", () => {
  const src = readFileSync("test/fixtures/mermaid/side-lane.mmd", "utf8");
  const art = render(src);
  assert.ok(art);
  const width = (label: string): number => {
    const line = art.plain.find((l) => l.includes(label)) ?? "";
    return line.lastIndexOf("│") - line.indexOf("│");
  };
  assert.equal(width("stripControls"), width("collapse subgraphs"));
  assert.notEqual(width("return null"), width("collapse subgraphs"));
});

test("a frame's members follow the order and rows of the peers declared before them", () => {
  // Q's boxes are taller than P's, so P's stack must open up to keep every pair level.
  const source =
    "flowchart LR\nsubgraph P\nX[X1]\nY[Y1]\nZ[Z1]\nend\nsubgraph Q\nA[A1 one two three four five six]\nB[B1]\nC[C1 one two three four five six]\nend\nA --> Z\nB --> Y\nC --> X\nS[S1] --> C";
  const plain = render(source)?.plain.join("\n") ?? "";
  const rowOf = (s: string) => plain.split("\n").findIndex((r) => r.includes(s));
  assert.ok(rowOf("C1") < rowOf("B1") && rowOf("B1") < rowOf("A1"), plain);
  const arrows = plain.split("\n").filter((r) => r.includes("▶"));
  assert.equal(arrows.length, 3, plain);
  for (const r of arrows) assert.match(r, /├─+[┼─]─+▶│ │ \w/, `edge jogs: ${r}`);
  assert.match(arrows[0], /S1 ├─+▶│ │ C1/, "a source sits level with its target inside the frame");
});

test("fence scanner round-trips and leaves quoted or indented-code Mermaid as text", () => {
  const quoted = "````markdown\n```mermaid\nflowchart LR\n A --> B\n```\n````\n";
  const indentedCode = "Text\n\n    ```mermaid\n    flowchart LR\n    ```\n";
  const mixed = `Intro\n\n~~~Mermaid extra\nflowchart LR\n A --> B\n~~~~\n\n- item\n\n  \`\`\`mermaid\n  flowchart LR\n   C --> D\n  \`\`\`\n\n\`\`\`mermaid\nflowchart LR\n E --> F`;
  for (const markdown of [quoted, indentedCode, mixed]) {
    assert.equal(
      scanFences(markdown)
        .map((segment) => segment.raw)
        .join(""),
      markdown,
    );
  }
  assert.deepEqual(
    [quoted, indentedCode].flatMap((markdown) => scanFences(markdown).filter((s) => s.mermaid)),
    [],
  );
  assert.deepEqual(
    scanFences(mixed).flatMap((segment) => (segment.mermaid ? [segment.mermaid] : [])),
    [
      { source: "flowchart LR\n A --> B", indent: "", closed: true, nested: false },
      { source: "flowchart LR\n C --> D", indent: "  ", closed: true, nested: true },
      { source: "flowchart LR\n E --> F", indent: "", closed: false, nested: false },
    ],
  );
});

test("a long return goes around the fan it arrives under: no crossing, no jog, one label", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/return-past-fan.mmd", import.meta.url), "utf8"),
    { maxWidth: 200 },
  );
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  assert.doesNotMatch(text, /[╫┼]/, "no crossing");
  assert.doesNotMatch(text, /└─+┐|┌─+┘/, "the return leaves its port straight");
  assert.equal(text.split("new not-ready answer").length, 2, "alike returns share one label");
  assert.match(text, /┌ polling ─*┴/, "a return leaves a frame past its title");
});

test("an order that draws a crossing gives way to one that draws none", () => {
  // The layered count calls the author's order crossing-free; drawn, the
  // return beside the fan crosses twice.
  const drawn = render(
    "flowchart TD\n  N2 -->|done| N3\n  N2 -->|ok| N1\n  N0 --> N2\n  N1 --> N3\n  N2 -->|fails| N0\n",
  );
  assert.ok(drawn);
  assert.doesNotMatch(drawn.plain.join("\n"), /╫/, "no crossing");
});

test("a crossing is kept over an order that runs a line behind a box", () => {
  const drawn = render(
    readFileSync(new URL("./fixtures/mermaid/lane-past-stack.mmd", import.meta.url), "utf8"),
    { maxWidth: 200 },
  );
  assert.ok(drawn);
  assertNoLineBehindBox(drawn.plain);
});

/** A line stopping dead at a border (no tee, no head) went behind the box. */
function assertNoLineBehindBox(plain: string[]): void {
  const rows = plain.map((row) => [...row]);
  rows.forEach((row, y) => {
    row.forEach((c, x) => {
      if (c !== "│") return;
      assert.notEqual(rows[y - 1]?.[x], "─", `line lost behind a box at ${x},${y}`);
      assert.notEqual(rows[y + 1]?.[x], "─", `line lost behind a box at ${x},${y}`);
    });
  });
}

test("LR lane with boxes stacked on both sides of its ends runs beside the rank, not behind a box", () => {
  for (const source of [
    "flowchart LR\n  N0 --> N1\n  N1 -->|fails| N2\n  N2 -->|fails| N3\n  N4 --> N5\n  N5 -->|timeout| N6\n  N6 -->|fails| N7\n  N6 -->|fails| N2\n  N6 -->|retry| N1\n  N1 -->|retry| N6\n  N5 --> N7\n  N8 --> N2\n",
    readFileSync(new URL("./fixtures/mermaid/class-lr-skips.mmd", import.meta.url), "utf8"),
  ]) {
    const drawn = render(source, { maxWidth: 200 });
    assert.ok(drawn);
    assertNoLineBehindBox(drawn.plain);
  }
});

test("LR lanes nest: the shorter span takes the inner track", () => {
  const drawn = render(
    "flowchart LR\n  N2 -->|retry| N3\n  N3 -->|retry| N4\n  N4 -->|timeout| N5\n  N5 --> N2\n  N4 -->|retry| N3\n",
  );
  assert.ok(drawn);
  assert.doesNotMatch(drawn.plain.join("\n"), /╫/, "no crossing");
});

test("a one-edge box stands over its entry and a skip-fed sink under its chain: no jog", () => {
  const drawn = render(
    'erDiagram\n  CUSTOMER ||--o{ ORDER : places\n  ORDER ||--|{ LINE_ITEM : contains\n  PRODUCT ||--o{ LINE_ITEM : "ordered in"\n  CUSTOMER }o--|| ADDRESS : "ships to"\n  ORDER }o--|| ADDRESS : "billed to"\n',
  );
  assert.ok(drawn);
  const rows = drawn.plain.map((row) => [...row]);
  // PRODUCT's line drops straight from its tee to its label row.
  const top = rows.findIndex((row) => row.join("").includes("PRODUCT")) + 1;
  const x = rows[top].lastIndexOf("┬");
  const end = rows.findIndex((row) => row.join("").includes("ordered in"));
  for (let y = top + 1; y <= end; y++)
    assert.equal(rows[y][x], "│", `PRODUCT's line bends at row ${y}`);
  // The skip into ADDRESS runs down one column from its turn to the box.
  const ships = rows.findIndex((row) => row.join("").includes("ships to"));
  const col = rows[ships].indexOf("│");
  const turn = rows.findLastIndex((row, y) => y < ships && row[col] === "┌");
  assert.ok(turn > 0);
  for (let y = turn + 1; y <= ships; y++)
    assert.equal(rows[y][col], "│", `the skip bends at row ${y}`);
});

test("sequence blocks bracket what they hold, nested one inside the other", () => {
  const drawn = render(
    "sequenceDiagram\n  A->>B: go\n  alt ok\n    B->>A: yes\n    loop retry\n      A->>B: again\n    end\n  else bad\n    B->>A: no\n  end\n",
  );
  assert.ok(drawn);
  const rows = drawn.plain;
  const at = (needle: string): number => rows.findIndex((row) => row.startsWith(needle));
  const [alt, loop, other] = [at("┌─ alt ok"), at("│ ┌─ loop retry"), at("├─ else bad")];
  assert.ok(alt >= 0 && loop > alt && other > loop, rows.join("\n"));
  const end = rows.findIndex((row, y) => y > other && row.startsWith("└─"));
  assert.ok(end > other, "the block closes");
  for (let y = alt + 1; y < end; y++)
    assert.match(rows[y], /^[│├]/, `the bracket breaks at row ${y}`);
  assert.ok(
    rows.some((row, y) => y > loop && y < other && row.startsWith("│ └─")),
    "the inner block closes inside",
  );
});

test("a return leaving a box's side tees off its border; verbless cardinalities read as a ratio", () => {
  const loop = render("flowchart TD\n  A --> B\n  B --> C\n  B --> D\n  D --> A\n  C --> E\n");
  assert.ok(loop);
  assert.doesNotMatch(loop.plain.join("\n"), /│─|─│/, "a line meets a border without a tee");
  const cls = render('classDiagram\n  Owner "1" o-- "*" Address\n');
  assert.ok(cls);
  assert.match(cls.plain.join("\n"), /1 : \*/);
});

test("a class body written on one line keeps its members", () => {
  const drawn = render("classDiagram\n  class Animal { +String name +eat() }\n");
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  assert.match(text, /\+String name/);
  assert.match(text, /\+eat\(\)/);
});

test("LR lanes whose spans interleave take opposite sides instead of crossing", () => {
  const drawn = render(
    "flowchart LR\n  N0 --> N1\n  N1 --> N2\n  N2 --> N3\n  N3 --> N4\n  N4 --> N5\n  N5 --> N6\n  N6 --> N7\n  N7 -->|done| N2\n  N3 --> N2\n  N3 -->|done| N1\n",
  );
  assert.ok(drawn);
  assert.doesNotMatch(drawn.plain.join("\n"), /╫/, "no crossing");
});

test("journey, kanban, gantt, quadrant, xychart, requirement and C4 sources draw", () => {
  const cases: [string, RegExp[]][] = [
    [
      "journey\n  title Day\n  section Work\n    Make tea: 5: Me\n    Do work: 1: Me, Cat\n",
      [/Make tea +█████ +5\/5 +Me/, /Do work +█░░░░ +1\/5 +Me, Cat/],
    ],
    [
      "kanban\n  Todo\n    [Write docs]\n  id2[Done]\n    id3[Ship it]@{ assigned: 'x' }\n",
      [/Todo +Done/, /• Write docs +• Ship it/],
    ],
    [
      "gantt\n  dateFormat YYYY-MM-DD\n  section S\n  First :a1, 2024-01-01, 10d\n  Second :after a1, 10d\n  Launch :milestone, 2024-01-21, 0d\n",
      [
        /First +█+░+ +2024-01-01 → 2024-01-11/,
        /Second +░+█+ +2024-01-11 → 2024-01-21/,
        /Launch +░+◆ +2024-01-21/,
      ],
    ],
    [
      "quadrantChart\n  x-axis Low --> High\n  y-axis Bad --> Good\n  quadrant-1 Expand\n  quadrant-3 Drop\n  A: [0.9, 0.9]\n",
      [/│ +│ Expand/, /│ Drop +│/, /● A|A ●/, /← Low +High →/, /↑ Good/],
    ],
    [
      'xychart-beta\n  x-axis [jan, feb]\n  y-axis "Sales" 0 --> 10\n  bar [5, 10]\n',
      [/jan +█{15}░{15} +5/, /feb +█{30} +10/],
    ],
    [
      "requirementDiagram\n  requirement need {\n  id: 1\n  risk: high\n  }\n  element part {\n  type: sim\n  }\n  part - satisfies -> need\n",
      [/«requirement»/, /risk: high/, /«element»/, /satisfies/],
    ],
    [
      'C4Container\n  title Internet banking\n  Boundary(b, "Bank") {\n    Person(u, "Customer", "Holds an account")\n    ContainerDb(db, "Ledger", "Postgres", "Stores balances")\n  }\n  Rel(u, db, "Reads", "SQL")\n  UpdateLayoutConfig($c4ShapeInRow="3")\n',
      [
        /┌ Internet banking ─/,
        /┌ Bank ─/,
        /│ +«person» +│/,
        /│ +Customer +│/,
        /│ Holds an account │/,
        /│ +\[Postgres\] +│/,
        /│ Stores balances │/,
        /Reads \[SQL\]/,
      ],
    ],
  ];
  for (const [source, expected] of cases) {
    const drawn = render(source);
    assert.ok(drawn, source);
    assert.deepEqual(drawn.warnings, [], source);
    for (const pattern of expected) assert.match(drawn.plain.join("\n"), pattern, source);
  }
  // An unreadable statement is dropped and named, not fatal.
  const lenient = render("gantt\n  First :2024-01-01, 10d\n  Broken :not-a-date, 3d\n");
  assert.ok(lenient);
  assert.match(lenient.warnings.join(), /Broken/);
});

test("a labelled skip beside a forward keeps its arrival order: no crossing, one jog", () => {
  const drawn = render(
    "flowchart TD\n  N0 --> N1\n  N1 --> N2\n  N2 -->|done| N3\n  N4 -->|done| N5\n  N0 -->|timeout| N2\n  N1 -->|ok| N4\n  N3 --> N2\n  N4 --> N0\n",
  );
  assert.ok(drawn);
  assert.ok(!drawn.plain.join("\n").includes("╫"));
  // The label sits left of its arrow, and the skip drops straight onto it.
  const head = drawn.plain.findIndex((row) => row.includes("timeout ▼"));
  const col = [...drawn.plain[head]].indexOf("▼");
  const turn = drawn.plain.findIndex((row) => [...row][col] === "┌");
  assert.ok(turn > 0);
  for (let y = turn + 1; y < head; y++) assert.equal([...drawn.plain[y]][col], "│");
  // The return out of N3 keeps its stem: the label beside it reads left.
  assert.ok(drawn.plain.some((row) => row.includes("done ▼ │")));
});

test("a return shares a border two cells off the stem, and its left label clears a passing chain", () => {
  const drawn = render(
    "stateDiagram-v2\n  N0 --> N1\n  N1 --> N2: timeout\n  N2 --> N3\n  N3 --> N1: retry\n  N3 --> N2\n  N0 --> N3\n  N2 --> N0: retry\n",
  );
  assert.ok(drawn);
  const art = drawn.plain.join("\n");
  // No line runs hard against another line's arrowhead.
  assert.doesNotMatch(art, /│[▲▼]|[▲▼]│/);
  // A chain passing left of a return's label stays whole beside it.
  const labelled = render(
    "flowchart TD\n  N0 -->|fails| N1\n  N2 --> N3\n  N4 --> N5\n  N5 -->|timeout| N3\n  N1 -->|retry| N2\n  N5 -->|retry| N2\n  N0 -->|timeout| N5\n  N3 -->|done| N5\n",
  );
  assert.ok(labelled);
  const row = labelled.plain.find((line) => line.includes("timeout ▲"));
  assert.ok(row?.trimStart().startsWith("│"));

  const across = render(
    "flowchart LR\n  N1 --> N2\n  N2 --> N3\n  N3 --> N0\n  N0 --> N4\n  N1 --> N3\n  N0 --> N3\n  N3 --> N4\n",
  );
  assert.ok(across);
  assert.doesNotMatch(across.plain.join("\n"), /│[▲▼]|[▲▼]│/);
});

test("a return's chain takes the port that keeps it clear of its target's stem", () => {
  const drawn = render(
    "flowchart TD\n  N0 -->|timeout| N1\n  N1 --> N2\n  N2 -->|ok| N3\n  N3 -->|fails| N4\n  N2 --> N1\n  N3 --> N0\n  N5 --> N1\n",
  );
  assert.ok(drawn);
  assert.ok(!drawn.plain.join("\n").includes("╫"));
  // The source stands over its drop: the return turns into its side.
  assert.ok(drawn.plain.some((row) => row.includes("─▶│ N0")));
  assert.ok(drawn.plain.some((row) => row.includes("▼ timeout ▼")));
});

test("a lane landing beside a departing stem takes the side that does not cut the nearer lane", () => {
  const drawn = render(
    "flowchart LR\n  N0 --> N1\n  N1 --> N2\n  N3 --> N4\n  N4 -->|done| N5\n  N0 -->|retry| N5\n  N5 -->|ok| N4\n  N1 -->|ok| N4\n",
  );
  assert.ok(drawn);
  assert.ok(!drawn.plain.join("\n").includes("╫"));
});

test("a lane leaves a frame's top beside its title, on the side its port was cut", () => {
  const drawn = render(
    "flowchart LR\n  subgraph G0[Order Service]\n    direction TB\n    N0\n    N1\n  end\n  subgraph G1[Order Service]\n    N2\n    N3\n  end\n  G0 ==>|retry| N3\n  N3 -.-> N7\n  G1 -->|timeout| G0\n  N7 -.-> N2\n",
  );
  assert.ok(drawn);
  // No frame title has a line or a head run into its text.
  for (const row of drawn.plain) assert.doesNotMatch(row, /┌ [^─]*[│┼╎▲▼┴┬][^─]* ─/);
});

test("a return leaves by the top when an arrival comes in from beyond its chain", () => {
  const drawn = render(
    "stateDiagram-v2\n  N1 --> N2: fails\n  N2 --> N3: done\n  N3 --> N4: ok\n  N4 --> N5: fails\n  N5 --> N6: timeout\n  N6 --> N7: fails\n  N7 --> N8: timeout\n  N5 --> N4: ok\n  N5 --> N2: timeout\n  N7 --> N0\n  N8 --> N5\n",
  );
  assert.ok(drawn);
  assert.ok(!drawn.plain.join("\n").includes("╫"));
});

test("a line break in a box label starts a new line; in an edge label it is a space", () => {
  const drawn = render("flowchart LR\n  A[First line<br/>second] -->|a<br>b| B\n");
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  assert.match(text, /│ First line ├/);
  assert.match(text, /│ +second +│/);
  assert.match(text, /a b/);
});

test("an xychart with a line series is a plot: bars as blocks, the line as joined dots", () => {
  const drawn = render(
    "xychart-beta\n  x-axis [jan, feb, mar]\n  y-axis 0 --> 100\n  bar [20, 100, 50]\n  line [100, 0, 100]\n",
  );
  assert.ok(drawn);
  const rows = drawn.plain;
  const top = rows.findIndex((row) => row.startsWith("100 ┤"));
  const bottom = rows.findIndex((row) => /^ +0 ┤/.test(row));
  assert.equal(bottom - top, 9);
  // Bars: 20 fills two rows of ten, 100 all ten.
  const col = (label: string): number => rows[bottom + 2].indexOf(label) + 1;
  const height = (x: number): number =>
    rows.slice(top, bottom + 1).filter((row) => row[x] === "█").length;
  assert.equal(height(col("jan")), 2);
  assert.equal(height(col("mar")), 5);
  assert.ok(height(col("feb")) >= 8);
  // The line starts on the top row and every plot row carries part of it.
  const braille = /[\u2801-\u28ff]/;
  assert.match(rows[top], braille);
  for (const row of rows.slice(top, bottom + 1)) assert.match(row, braille);
  assert.match(rows.at(-1) ?? "", /█ bar +⠒ line/);
});

test("a fan-out branch carries its label at its own arrival, not on a sibling's row", () => {
  const drawn = render("flowchart LR\n  A --> B\n  A -->|ab| C\n");
  assert.ok(drawn);
  const named = drawn.plain.findIndex((row) => row.includes("ab"));
  assert.match(drawn.plain[named + 1], /▶│ C │/);
});

test("a node first used outside a subgraph joins the subgraph that lists it later", () => {
  const drawn = render("flowchart LR\n  X --> A\n  subgraph G[Group]\n    A --> B\n  end\n");
  assert.ok(drawn);
  const text = drawn.plain.join("\n");
  assert.match(text, /┌ Group ─/);
  // X stays outside the frame; A and B stand inside it.
  assert.match(text, /│ X ├─+▶│ │ A ├─+▶│ B │ │/);
});

test("an edge label sits against its own line, never a neighbour's", () => {
  const text = (source: string): string => {
    const drawn = render(source);
    assert.ok(drawn);
    return drawn.plain.join("\n");
  };
  // Both ends shared: the label goes beside the edge's own leg.
  const fan = text(
    "flowchart LR\n  N0 --> N1\n  N1 --> N2\n  N1 -->|fails| N6\n  N3 --> N6\n  N6 --> N4",
  );
  assert.match(fan, /│ fails\n/, "beside the leg, not over the sibling's arrival");
  // Two edges between the same frames at different boxes are not parallel:
  // each keeps its own label.
  const frames = text(
    "flowchart LR\n  subgraph G0\n    N0\n    N1\n  end\n  subgraph G1\n    N2\n    N3\n  end\n  G1 ==> N1\n  N2 --o|ships| N0",
  );
  assert.match(
    frames,
    /─o│[^\n]*\n[^\n]*│ +ships /,
    "the label stays under the circle-headed edge, off the thick one",
  );
  // Self-loops on one box share one hook, so one label names them all.
  const loops = text(
    "classDiagram\n  direction LR\n  N5 o-- N5 : owns\n  N5 *-- N5 : holds\n  N4 <|-- N5",
  );
  assert.match(loops, /▲ owns \/ holds/);
  // A self-loop's label stays inside its column, clear of the next band's lines.
  const loop = text("flowchart LR\n  N5 --> N3\n  N3 --> N0\n  N3 --> N6\n  N3 -->|again| N3");
  assert.match(loop, /▲ again +│/, "the bus runs past the label, not through it");
});

test("top-down edges into different boxes of one frame each arrive over their own box", () => {
  const text = (source: string): string => {
    const drawn = render(source);
    assert.ok(drawn);
    return drawn.plain.join("\n");
  };
  const fan = text("flowchart TD\n  A --> X\n  A --> Z\n  subgraph G[Group]\n    X\n    Z\n  end");
  assert.match(
    fan,
    /┌─+┴─+┐\n +▼ +▼\n┌ Group ─+┐\n│ +┌───┐ +┌───┐/,
    "two heads, one over each box",
  );
  // Sources with one edge each stand over the box they point at: no bend.
  const pair = text("flowchart TD\n  A --> X\n  C --> Z\n  subgraph G[Group]\n    X\n    Z\n  end");
  assert.doesNotMatch(pair, /[┘└┐┌]\n +▼/, "both drops run straight");
  // With room between the border and the box, the edge runs through to the box.
  const nested = text(
    "flowchart TD\n  A --> X\n  C --> W\n  subgraph Outer\n    subgraph Inner\n      X --> Y\n    end\n    W\n  end",
  );
  assert.match(nested, /┌ Outer ─+┼─+┐/, "the border is pierced over W");
  assert.match(nested, /▼ +│\n[^\n]*┌───┐ │\n[^\n]*│ W │/, "and the head lands on W");
});
