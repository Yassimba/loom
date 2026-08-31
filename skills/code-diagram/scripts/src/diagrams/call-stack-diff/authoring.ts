import { z } from "zod";
import {
  noChildrenSchema,
  nonEmptyStringSchema,
  optionalNonEmptyStringSchema,
  peekableAnchorRefSchema,
  type PeekableAnchorRef,
} from "../../authoring/core";

export const callsAssertionSchema = z.strictObject({
  __kind: z.literal("call-assertion"),
  parent: peekableAnchorRefSchema,
  child: peekableAnchorRefSchema,
  reason: optionalNonEmptyStringSchema,
});
export type CallsAssertion = z.infer<typeof callsAssertionSchema>;

export function calls(
  parent: PeekableAnchorRef,
  child: PeekableAnchorRef,
  reason?: string,
): CallsAssertion {
  peekableAnchorRefSchema.parse(parent);
  peekableAnchorRefSchema.parse(child);
  if (reason !== undefined) nonEmptyStringSchema.parse(reason);
  return Object.freeze({
    __kind: "call-assertion",
    parent,
    child,
    ...(reason === undefined ? {} : { reason }),
  });
}

export const callStackEntrySchema = z.union([
  peekableAnchorRefSchema,
  callsAssertionSchema,
]);
export type CallStackEntry = z.infer<typeof callStackEntrySchema>;
export function isCallsAssertion(
  value: CallStackEntry,
): value is CallsAssertion {
  return value.__kind === "call-assertion";
}
export function callStackEntryAnchor(
  entry: CallStackEntry,
): PeekableAnchorRef {
  return isCallsAssertion(entry) ? entry.child : entry;
}
export const callStackDiffPropsSchema = z
  .strictObject({
    title: optionalNonEmptyStringSchema,
    base: z.array(callStackEntrySchema),
    head: z.array(callStackEntrySchema),
    children: noChildrenSchema,
  })
  .superRefine((value, context) => {
    if (value.base.length === 0 && value.head.length === 0) {
      context.addIssue({
        code: "custom",
        path: ["head"],
        message: "Must list at least one frame on base or head",
      });
    }
    const headIds = new Set(
      value.head.map((entry) => callStackEntryAnchor(entry).id),
    );
    value.head.forEach((entry, index) => {
      const anchor = callStackEntryAnchor(entry);
      if (anchor.peek.props.graph === "base")
        context.addIssue({
          code: "custom",
          path: ["head", index],
          message: `Anchor "${anchor.id}" points at base; a head frame must point at head`,
        });
    });
    value.base.forEach((entry, index) => {
      const anchor = callStackEntryAnchor(entry);
      if (anchor.peek.props.graph !== "base" && !headIds.has(anchor.id))
        context.addIssue({
          code: "custom",
          path: ["base", index],
          message: `Anchor "${anchor.id}" is a removed frame; give it graph: "base" so it points at the old code`,
        });
    });
  });
export type CallStackDiffProps = z.infer<typeof callStackDiffPropsSchema>;
