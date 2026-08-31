import { z } from "zod";

export const nonEmptyStringSchema = z
  .string()
  .refine((value) => value.trim().length > 0, "Must not be empty");
export const optionalNonEmptyStringSchema = nonEmptyStringSchema.optional();
export const noChildrenSchema = z.never().optional();

export interface SourceLine {
  number: number;
  text: string;
}

export interface CodePeekResolution {
  source: {
    file: string;
    fromLine: number;
    toLine: number;
    lines: SourceLine[];
  };
}

export const actorInputSchema = z.strictObject({
  label: nonEmptyStringSchema,
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type ActorInput = z.infer<typeof actorInputSchema>;
export const actorInputMapSchema = z.record(
  nonEmptyStringSchema,
  actorInputSchema,
);
export type ActorInputMap = z.infer<typeof actorInputMapSchema>;
export const actorRefSchema = z.strictObject({
  __kind: z.literal("db-actor-ref"),
  id: nonEmptyStringSchema,
  label: nonEmptyStringSchema,
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type ActorRef = z.infer<typeof actorRefSchema>;

export const codePeekRangeInputSchema = z
  .strictObject({
    file: nonEmptyStringSchema,
    fromLine: z.int().positive(),
    toLine: z.int().positive(),
    theme: z.enum(["system", "light", "dark"]).optional(),
    graph: z.enum(["head", "base"]).optional(),
    children: noChildrenSchema,
  })
  .refine((value) => value.toLine >= value.fromLine, {
    path: ["toLine"],
    message: "Must be greater than or equal to fromLine",
  });
export type CodePeekProps = z.infer<typeof codePeekRangeInputSchema>;
export const codePeekRefSchema = z.strictObject({
  __kind: z.literal("code-peek-ref"),
  props: codePeekRangeInputSchema,
  resolution: z.custom<CodePeekResolution>().nullable(),
});
export type CodePeekRef = z.infer<typeof codePeekRefSchema>;
export const anchorInputSchema = z.strictObject({
  title: nonEmptyStringSchema,
  peek: codePeekRangeInputSchema.optional(),
  detail: optionalNonEmptyStringSchema,
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type AnchorInput = z.infer<typeof anchorInputSchema>;
export const anchorInputMapSchema = z.record(
  nonEmptyStringSchema,
  z.union([nonEmptyStringSchema, anchorInputSchema]),
);
export type AnchorInputMap = z.infer<typeof anchorInputMapSchema>;
export const anchorRefSchema = z.strictObject({
  __kind: z.literal("db-anchor-ref"),
  id: nonEmptyStringSchema,
  title: nonEmptyStringSchema,
  detail: optionalNonEmptyStringSchema,
  peek: codePeekRefSchema.optional(),
  softwareMapPath: optionalNonEmptyStringSchema,
});
export type AnchorRef = z.infer<typeof anchorRefSchema>;
export const peekableAnchorRefSchema = anchorRefSchema.extend({
  peek: codePeekRefSchema,
});
export type PeekableAnchorRef = z.infer<typeof peekableAnchorRefSchema>;
export type AnchorRefFor<T extends AnchorInputMap[string]> = AnchorRef &
  (T extends { peek: infer Peek extends CodePeekProps }
    ? { peek: CodePeekRef & { props: Peek } }
    : unknown);

export function defineActors<T extends ActorInputMap>(
  input: T,
): { [K in keyof T]: ActorRef } {
  actorInputMapSchema.parse(input);
  return Object.fromEntries(
    Object.entries(input).map(([id, actor]) => [
      id,
      Object.freeze({
        __kind: "db-actor-ref",
        id,
        label: actor.label,
        softwareMapPath: actor.softwareMapPath,
      } satisfies ActorRef),
    ]),
  ) as { [K in keyof T]: ActorRef };
}

export function defineAnchors<T extends AnchorInputMap>(
  input: T,
): { [K in keyof T]: AnchorRefFor<T[K]> } {
  const anchors = anchorInputMapSchema.parse(input);
  return Object.fromEntries(
    Object.entries(anchors).map(([id, rawAnchor]) => {
      const anchor =
        typeof rawAnchor === "string"
          ? { title: rawAnchor }
          : (rawAnchor as AnchorInput);
      const peek = anchor.peek
        ? Object.freeze({
          __kind: "code-peek-ref",
          props: anchor.peek,
          resolution: null,
        } satisfies CodePeekRef)
        : undefined;
      return [
        id,
        Object.freeze({
          __kind: "db-anchor-ref",
          id,
          ...anchor,
          peek,
        } satisfies AnchorRef),
      ];
    }),
  ) as { [K in keyof T]: AnchorRefFor<T[K]> };
}

export function throwAuthoringIssue(
  path: PropertyKey[],
  message: string,
): never {
  throw new z.ZodError([
    { code: "custom", path, message, input: undefined },
  ]);
}
