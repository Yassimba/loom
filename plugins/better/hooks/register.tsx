import type { Register } from "claude-code";
import { atom, read, update } from "claude-code";

const HELPER = "better:helper";

// The four questions are Felix Rieseberg's "Depth" prompt; the working rules
// follow pi-better's helper.md (MIT, Nico Bailon).
const HELPER_PROMPT = `You rewrite a prompt into a deeper version of the same task. You never carry the prompt out.

First, without asking anything, find and read the code, files, docs and logs the prompt touches, so your answers rest on them. The user wants a deeper version of the task or plan request, not a bigger one. Before rewriting it: (1) say what a top professional in this field would do here that the prompt does not ask for; (2) name the rawest material they would start from instead of what has already been digested; (3) name the specific methods and tools they would use; (4) say what, other than you or the user, would show the result is right. Then rewrite the prompt to ask for exactly that.

## How to work

1. Gather the raw material. Do not rely on summaries when the source is reachable. Keep every search quick and inside the project and the paths the prompt names.
2. Answer (1) to (4) in a few lines each, using concrete facts from what you found: file names, counts, real names of components, commands and tools that exist here.
3. Rewrite the prompt so it asks for exactly that. Keep the user's goal and voice, put the concrete facts in, and say what proves the result is right. Write it as the user's own request, ready to send as is.

End your reply with the rewritten prompt between <improved-prompt> and </improved-prompt>, and nothing after it.

Never edit files, start the work, or launch anything. You only read and rewrite.`;

const RESTATE =
  "Restate the task I am working on now as one prompt I could send to start it again, including the feedback I have given so far. Reply with the prompt only.";

const run = atom({ plugin: "better", key: "run" } as const, null);

export const register: Register = (on) => {
  on("session.start", async ($, e, next) => {
    await $.command.register({
      name: "better",
      description: "Rewrite a prompt into a deeper version, checked against your files",
      argumentHint: "[prompt | stop]",
      immediate: true,
    });
    await $.agent.register({
      name: "helper",
      description: "Rewrites a prompt for /better",
      prompt: HELPER_PROMPT,
      tools: ["Read", "Grep", "Glob"],
      model: "inherit",
    });

    return next(e);
  });

  // Only /better starts the helper; the model is not offered it.
  on("agent.offer", { agent: HELPER }, () => ({ isOffered: false }));

  on("command.run", { command: "better" }, async ($, e) => {
    const current = await read($, run);
    const args = e.args.trim();

    if (args === "stop") {
      if (current === null) {
        return { text: "/better is not running." };
      }

      await update($, run, () => null);
      await $.tool.call({ tool: "TaskStop", task_id: current.agentId }).catch(() => undefined);

      return { text: "/better stopped." };
    }

    if (current !== null) {
      return { text: "/better is already running. `/better stop` cancels it." };
    }

    let task = args;

    if (task === "") {
      const restated = await $.model.fork({ prompt: RESTATE });

      if (!restated.isAnswered) {
        return {
          text: `/better failed: could not restate the current task (${restated.reason}). Run \`/better <prompt>\`.`,
        };
      }

      task = restated.text;
    }

    const { agentId, deny } = await $.agent.spawn({
      subagentType: HELPER,
      description: "Deepen the prompt",
      prompt: `Rewrite this prompt:\n\n<prompt>\n${task}\n</prompt>`,
    });

    if (agentId === undefined) {
      return { text: `/better failed: ${deny ?? "the helper did not start"}` };
    }

    await update($, run, () => ({ agentId, tools: 0, last: "" }));

    return {
      text: "Working on a deeper prompt. It lands in your prompt box. `/better stop` cancels it.",
    };
  });

  on("tool.call", async ($, e, next) => {
    // Progress only: the main loop's calls skip it, and a failed count never
    // holds a tool call back.
    if (e.agentId !== undefined) {
      await update($, run, (r) =>
        r !== null && r.agentId === e.agentId ? { ...r, tools: r.tools + 1, last: e.tool } : r,
      ).catch(() => undefined);
    }

    return next(e);
  });

  on("turn.complete", async ($, e, next) => {
    const current = await read($, run);

    if (current === null || current.agentId !== e.agentId) {
      return next(e);
    }

    await update($, run, () => null);
    const improved = /<improved-prompt>([\s\S]*?)<\/improved-prompt>/.exec(e.answer)?.[1]?.trim();

    if (!improved) {
      $.ui.toast(`/better failed: no improved prompt (the helper ended with "${e.reason}").`);

      return next(e);
    }

    // What the person typed meanwhile stays, below the new prompt.
    const box = await $.prompt.read();
    const text = box.text.trim() === "" ? improved : `${improved}\n\n${box.text}`;
    const { isFilled } = await $.prompt.fill({ text });
    $.ui.toast(
      isFilled
        ? "The improved prompt is in your prompt box. Edit it, then press Enter."
        : "/better: could not fill the prompt box. Close the open dialog and run /better again.",
    );

    return next(e);
  });

  on("ui.render", { component: "AbovePrompt" }, async ($, e, next) => {
    const current = await read($, run);

    if (e.props.hasSurvey || current === null) {
      return next(e);
    }

    const { Box, Text } = $.ui.resolve(e);

    return (
      <Box>
        <Text dimColor>
          /better: reading your code and files ({current.tools} tool calls
          {current.last && `, last ${current.last}`}) · /better stop cancels
        </Text>
      </Box>
    );
  });
};
