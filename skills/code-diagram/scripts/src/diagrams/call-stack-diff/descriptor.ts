import {
  callStackDiffPropsSchema,
  callStackEntryAnchor,
  isCallsAssertion,
  type CallStackDiffProps,
} from "./authoring";
import { resolveAnchorEvidence } from "../../document/anchor-evidence";
import type { ReviewSurfaceDescriptor } from "../../document/model";
import {
  callStackBrowserSchema,
  callStackEvidenceErrors,
  diffCallStacks,
} from "./model";

export const callStackDiffSurfaceDescriptor = {
  kind: "call-stack-diff",
  source: {
    type: "mdx",
    createComponents: (emit) => ({
      CallStackDiff(props) {
        return emit(callStackDiffPropsSchema.parse(props));
      },
    }),
  },
  capturedSchema: callStackDiffPropsSchema,
  async compile(model: CallStackDiffProps, evidence) {
    const rows = diffCallStacks(model.base, model.head);
    const errors = callStackEvidenceErrors(rows, (file, side) =>
      evidence.changedLines(file, side),
    );
    if (errors.length)
      throw new Error(`CALL_STACK_EVIDENCE_INVALID: ${errors.join("\n")}`);
    return {
      title: model.title,
      rows: await Promise.all(
        rows.map(async (row) => {
          if (isCallsAssertion(row.entry))
            await resolveAnchorEvidence(row.entry.parent, evidence);
          const resolved = await resolveAnchorEvidence(callStackEntryAnchor(row.entry), evidence);
          return { ...row, source: resolved.source };
        }),
      ),
    };
  },
  compiledSchema: callStackBrowserSchema,
  browser: { specifier: "./src/diagrams/call-stack-diff/viewer.tsx" },
} satisfies ReviewSurfaceDescriptor<
  "call-stack-diff",
  CallStackDiffProps,
  ReturnType<typeof callStackBrowserSchema.parse>
>;
