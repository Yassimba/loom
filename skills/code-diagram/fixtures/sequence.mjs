import { defineDocument } from "../scripts/authoring.mjs";

export default defineDocument({
  version: 1,
  title: "Submission flow",
  intro: ["A request moves from the caller through the API and into the work queue."],
  diagrams: [
    {
      type: "sequence",
      label: "Submit and process",
      actors: {
        caller: { label: "Caller" },
        api: { label: "API" },
        queue: { label: "Work queue" },
      },
      messages: [
        {
          from: "caller",
          to: "api",
          label: "submit(input)",
          evidence: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 2, toLine: 4 },
        },
        {
          from: "api",
          to: "queue",
          label: "enqueue(id)",
          evidence: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 7, toLine: 9 },
        },
        {
          from: "queue",
          to: "queue",
          label: "retry failed job",
          code: { language: "ts", text: "attempts += 1" },
        },
        {
          from: "caller",
          to: "api",
          label: "poll(id)",
          evidence: { file: "skills/code-diagram/fixtures/source.ts", fromLine: 11, toLine: 11 },
        },
      ],
    },
  ],
});
