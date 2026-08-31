import { z } from "zod";
import {
  actorRefSchema,
  anchorRefSchema,
  noChildrenSchema,
  nonEmptyStringSchema,
  optionalNonEmptyStringSchema,
  peekableAnchorRefSchema,
} from "../../authoring/core";

const inlineSequenceActorSchema = z.strictObject({
  id: optionalNonEmptyStringSchema,
  label: nonEmptyStringSchema,
});
export const sequenceActorInputSchema = z.union([
  actorRefSchema,
  inlineSequenceActorSchema,
]);
export type SequenceActorInput = z.infer<typeof sequenceActorInputSchema>;
export const sequenceMessageCodeInputSchema = z.union([
  nonEmptyStringSchema,
  z.strictObject({
    language: optionalNonEmptyStringSchema,
    text: nonEmptyStringSchema,
  }),
]);
export type SequenceMessageCodeInput = z.infer<
  typeof sequenceMessageCodeInputSchema
>;
const sequenceMessageBaseShape = {
  from: sequenceActorInputSchema,
  to: sequenceActorInputSchema,
  label: nonEmptyStringSchema,
};
export const sequenceMessageInputSchema = z.union([
  z.strictObject({
    ...sequenceMessageBaseShape,
    anchor: peekableAnchorRefSchema,
    code: sequenceMessageCodeInputSchema.optional(),
  }),
  z.strictObject({
    ...sequenceMessageBaseShape,
    anchor: anchorRefSchema.optional(),
    code: sequenceMessageCodeInputSchema,
  }),
]);
export type SequenceMessageInput = z.infer<typeof sequenceMessageInputSchema>;
export const sequenceDiagramPropsSchema = z.strictObject({
  label: nonEmptyStringSchema,
  messages: z.array(sequenceMessageInputSchema).min(1),
  children: noChildrenSchema,
});
export type SequenceDiagramProps = z.infer<typeof sequenceDiagramPropsSchema>;
