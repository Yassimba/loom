import { z } from "zod";
import { sequenceDiagramPropsSchema } from "./authoring";

export const sequenceBrowserSchema = sequenceDiagramPropsSchema;
export const callStackBrowserSchema = z.strictObject({
  title: z.string().optional(),
  rows: z.array(
    z.strictObject({
      entry: z.any(),
      change: z.enum(["added", "removed", "unchanged"]),
      depth: z.int().nonnegative(),
      source: z.strictObject({
        file: z.string(),
        fromLine: z.int(),
        toLine: z.int(),
        lines: z.array(z.strictObject({ number: z.int(), text: z.string() })),
      }),
    }),
  ),
});
