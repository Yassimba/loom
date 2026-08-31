import {
  sequenceDiagramPropsSchema,
  type SequenceDiagramProps,
} from "./authoring";
import { resolveAnchorEvidence } from "../../document/anchor-evidence";
import type { ReviewSurfaceDescriptor } from "../../document/model";
import { createSequence } from "./model";

export const sequenceSurfaceDescriptor = {
  kind: "sequence",
  source: {
    type: "mdx",
    createComponents: (emit) => ({
      SequenceDiagram(props) {
        const parsed = sequenceDiagramPropsSchema.parse(props);
        createSequence(parsed);
        return emit(parsed);
      },
    }),
  },
  capturedSchema: sequenceDiagramPropsSchema,
  async compile(model: SequenceDiagramProps, evidence) {
    return {
      ...model,
      messages: await Promise.all(
        model.messages.map(async (message) => {
          const anchor = message.anchor;
          if (!anchor?.peek) return message;
          return {
            ...message,
            anchor: (await resolveAnchorEvidence({ ...anchor, peek: anchor.peek }, evidence)).anchor,
          };
        }),
      ),
    };
  },
  compiledSchema: sequenceDiagramPropsSchema,
  browser: { specifier: "./src/diagrams/sequence/viewer.tsx" },
} satisfies ReviewSurfaceDescriptor<"sequence", SequenceDiagramProps, SequenceDiagramProps>;
