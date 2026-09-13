import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
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
  assert.match(text, /│ RDF Extension ├─┬─┼─+▶│/, "dotted fork, then the trunk join");
  // Two private arrivals into one target must share the approach or
  // cross (PrivateFanIn.lean): the dotted arm drops to Turbine's row
  // and joins its fan there, no hop.
  assert.doesNotMatch(text, /╫/, "no hop");
  assert.match(
    text,
    /optional +│ SHACL Check Engine/,
    "the label sits above its own departure, not at the join",
  );
  assert.match(text, /│ Turbine Engine ├─┴─┴─+▶│ compares structure/, "one head for both");
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
