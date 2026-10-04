import { expect, type TestBody, test } from "claude-code/testing";

const SURFACES = ["terminal", "desktop", "vscode", "mobile"] as const;
const mounted = (text: string) => ({
  plugin: "loom-mermaid",
  component: "AssistantMessage" as const,
  props: { text, isFirstOfReply: true },
});

test("a Mermaid fence in a reply draws as a diagram on every surface", async ($) => {
  const text =
    "Before\n\n```mermaid\nflowchart LR\n A[Start]:::green --> B[End]\n```\n\nAfter\n\n- item\n\n  ```mermaid\n  flowchart LR\n   C[Nested] --> D\n  ```\n";
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...mounted(text), surface });
    await ui.drawn();
    expect(await ui.find({ type: "Markdown", text: /Before/ })).toBeDefined();
    expect(await ui.find({ type: "Markdown", text: /After/ })).toBeDefined();
    expect(await ui.find({ type: "Markdown", text: /flowchart/ })).toBeUndefined();
    expect(await ui.find({ type: "Text", text: /Start/ })).toBeDefined();
    expect(await ui.find({ type: "Text", text: /^ {2}.*Nested/ })).toBeDefined();
    await ui.unmount();
  }
});

test("a fence still arriving draws its complete statements and holds a half-written label", async ($) => {
  const arriving = "```mermaid\nflowchart LR\n A[First] --> B[Second]\n B --> C[Thi";
  const ui = await $.ui.mount({ ...mounted(arriving), surface: "terminal" });
  expect(await ui.find({ type: "Text", text: /Second/ })).toBeDefined();
  expect(await ui.find({ type: "Text", text: /Thi/ })).toBeUndefined();
  await ui.redraw({ text: "```mermaid\nflowchart", isFirstOfReply: true });
  expect(await ui.find({ type: "Text", text: /Drawing Mermaid/ })).toBeDefined();
  await ui.unmount();
});

test("a reply with no drawable Mermaid is left to the engine", async ($, on) => {
  on("ui.render", { component: "AssistantMessage" }, ($, e) => {
    const { Text } = $.ui.resolve(e);
    return <Text>engine drew this</Text>;
  });
  for (const text of ["Plain prose.", "```mermaid\nnot a diagram\n```\n"]) {
    const ui = await $.ui.mount({ ...mounted(text), surface: "terminal" });
    expect(await ui.find({ type: "Text", text: /engine drew this/ })).toBeDefined();
    await ui.unmount();
  }
});

test("a clickable node draws as a link and a styled node takes its stroke color", async ($) => {
  const text =
    '```mermaid\nflowchart LR\n A[Docs]:::custom --> B\n classDef custom stroke:#abcdef\n click A "https://example.com"\n```\n';
  for (const surface of SURFACES) {
    const ui = await $.ui.mount({ ...mounted(text), surface });
    const tree = JSON.stringify(await ui.drawn());
    expect(tree).toContain("https://example.com");
    expect(tree).toContain("#abcdef");
    await ui.unmount();
  }
});

test("the system prompt gains the Mermaid note after the engine's sections", async ($, on) => {
  on("prompt.compose", () => ({ sections: [{ id: "intro", text: "Intro", scope: "shared" }] }));
  const { sections } = await $.prompt.compose({
    model: "m",
    promptModel: "m",
    surfaces: ["terminal"],
    tools: [],
    outputStyle: null,
    traits: [],
  });
  expect(sections.map((section) => section.id)).toEqual(["intro", "loom-mermaid:guidance"]);
  expect(sections[1]?.text).toContain(":::green");
});

const stream = async ($: Parameters<TestBody>[0], batches: string[]) => {
  const displayed = [];
  for (const [index, delta] of batches.entries()) {
    const result = await $.classic.MessageDisplay({
      turn_id: "turn",
      message_id: "message",
      index,
      final: false,
      delta,
    });
    displayed.push(result.displayContent ?? delta);
  }
  return displayed;
};

test("a streaming reply notes a fence when it opens and draws it when it closes", async ($, on) => {
  on("classic.MessageDisplay", () => ({}));
  const displayed = await stream($, [
    "Before\n\n",
    "```mermaid\nflowchart LR\n",
    " A[Start] --> B[End]\n",
    "```\n\nAfter\n",
  ]);
  expect(displayed.slice(0, 3)).toEqual(["Before\n\n", "_Drawing Mermaid…_\n\n", ""]);
  expect(displayed[3]).toContain("Start");
  expect(displayed[3]).not.toContain("flowchart");
  expect(displayed[3]?.endsWith("\nAfter\n")).toBe(true);
});

test("the band above the prompt grows with the open fence and clears when it closes", async ($, on) => {
  on("classic.MessageDisplay", () => ({}));
  on("ui.render", { component: "AbovePrompt" }, ($, e) => {
    const { Text } = $.ui.resolve(e);
    return <Text>engine band</Text>;
  });
  for (const surface of ["terminal", "desktop"] as const) {
    const ui = await $.ui.mount({
      plugin: "loom-mermaid",
      surface,
      component: "AbovePrompt",
      props: {
        hasSurvey: false,
        isWorking: true,
        maxRows: 20,
        bodyColumns: 80,
        scroll: { offset: 0, bodyRows: 20 },
        view: {},
      },
    });
    expect(await ui.find({ type: "Text", text: /engine band/ })).toBeDefined();
    await stream($, ["```mermaid\nflowchart"]);
    expect(await ui.find({ type: "Text", text: /Drawing Mermaid/ })).toBeDefined();
    await stream($, [" LR\n A[First] --> B[Second]\n B --> C[Thi"]);
    expect(await ui.find({ type: "Text", text: /Second/ })).toBeDefined();
    expect(await ui.find({ type: "Text", text: /Thi/ })).toBeUndefined();
    await stream($, ["rd]\n```\n"]);
    expect(await ui.find({ type: "Text", text: /engine band/ })).toBeDefined();
    await ui.unmount();
  }
});
