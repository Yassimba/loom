import {
  defineActors,
  defineAnchors,
} from "virtual:progressive-review-authoring";

export const actors = defineActors({
  caller: { label: "Caller" },
  api: { label: "API" },
  queue: { label: "Queue" },
});

export const anchors = defineAnchors({
  submit: {
    title: "Submit request",
    peek: {
      file: "skills/code-diagram/fixtures/source.ts",
      fromLine: 2,
      toLine: 4,
    },
  },
  enqueue: {
    title: "Enqueue request",
    peek: {
      file: "skills/code-diagram/fixtures/source.ts",
      fromLine: 6,
      toLine: 8,
    },
  },
});

export const messages = [
  {
    from: actors.caller,
    to: actors.api,
    label: "submit(input)",
    anchor: anchors.submit,
  },
  {
    from: actors.api,
    to: actors.queue,
    label: "enqueue(id)",
    anchor: anchors.enqueue,
  },
];
