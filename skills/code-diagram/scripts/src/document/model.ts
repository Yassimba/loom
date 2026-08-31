import type { ComponentType, ReactElement } from "react";
import { z } from "zod";
import type { CodePeekProps } from "../authoring/core";

export const sourceLineSchema = z.strictObject({ number: z.int().positive(), text: z.string() });
export const sourceRangeSchema = z.strictObject({
  file: z.string().min(1),
  fromLine: z.int().positive(),
  toLine: z.int().positive(),
  lines: z.array(sourceLineSchema),
});
export type SourceRange = z.infer<typeof sourceRangeSchema>;

export const changeStateSchema = z.enum([
  "added",
  "removed",
  "modified",
  "unchanged",
]);
export type ChangeState = z.infer<typeof changeStateSchema>;
export const diffStateSchema = z.enum(["added", "removed", "unchanged"]);
export type DiffState = z.infer<typeof diffStateSchema>;
export type EmphasisState = "normal" | "active" | "inactive";

export type OpenEvidence = (
  source: SourceRange | readonly SourceRange[],
  title?: string,
) => void;

export interface DiagramRendererProps<Model> {
  model: Model;
  openEvidence: OpenEvidence;
}

export const compiledSurfaceSchema = z.strictObject({
  kind: z.string().min(1),
  model: z.unknown(),
});
export type CompiledSurface = z.infer<typeof compiledSurfaceSchema>;
export const compiledDocumentSchema = z.strictObject({
  version: z.literal(1),
  title: z.string().min(1),
  diagrams: z.array(compiledSurfaceSchema),
});
export type CompiledDocument = z.infer<typeof compiledDocumentSchema>;

export const browserDocumentSchema = compiledDocumentSchema.omit({ title: true });

export interface SourceEvidenceResolver {
  resolveRange(input: CodePeekProps): Promise<SourceRange>;
  changedLines(
    file: string,
    side: "base" | "head",
  ): { deleted: ReadonlySet<number>; added: ReadonlySet<number> } | null;
}

export type BrowserAsset = "libavoid";

export interface BrowserRegistration {
  specifier: string;
  assets?: readonly BrowserAsset[];
}

export type SurfaceSource<Captured> =
  | {
      type: "mdx";
      createComponents(
        emit: (model: Captured) => ReactElement,
      ): Record<string, ComponentType<Record<string, unknown>>>;
    }
  | {
      type: "artifact";
      fileName: string;
      typecheck?: {
        moduleAliases?: Readonly<Record<string, string>>;
      };
      collect(path: string, repo: string): Promise<Captured[]>;
    };

export interface ReviewSurfaceDescriptor<Kind extends string, Captured, Compiled> {
  kind: Kind;
  source: SurfaceSource<Captured>;
  capturedSchema: z.ZodType<Captured>;
  compile(model: Captured, evidence: SourceEvidenceResolver): Promise<Compiled>;
  compiledSchema: z.ZodType<Compiled>;
  validateDocument?(surfaces: readonly CompiledSurface[]): void;
  browser: BrowserRegistration;
}

export type AnySurfaceDescriptor = ReviewSurfaceDescriptor<string, any, any>;
