import type { ComponentType, ReactElement } from "react";
import { z } from "zod";

export const sourceLineSchema = z.strictObject({ number: z.int().positive(), text: z.string() });
export const sourceRangeSchema = z.strictObject({
  file: z.string().min(1),
  fromLine: z.int().positive(),
  toLine: z.int().positive(),
  lines: z.array(sourceLineSchema),
});
export type SourceRange = z.infer<typeof sourceRangeSchema>;

export const compiledSurfaceSchema = z.strictObject({
  kind: z.string().min(1),
  model: z.unknown(),
});
export type CompiledSurface = z.infer<typeof compiledSurfaceSchema>;
export const compiledDocumentSchema = z.strictObject({
  version: z.literal(1),
  title: z.string().min(1),
  html: z.string(),
  repo: z.string().min(1),
  revision: z.string().nullable(),
  diagrams: z.array(compiledSurfaceSchema),
});
export type CompiledDocument = z.infer<typeof compiledDocumentSchema>;

export interface SourceEvidenceResolver {
  resolveRange(input: {
    file: string;
    fromLine: number;
    toLine: number;
    graph?: "head" | "base";
  }): Promise<SourceRange>;
  changedLines(
    file: string,
    side: "base" | "head",
  ): { deleted: ReadonlySet<number>; added: ReadonlySet<number> } | null;
}

export interface BrowserRegistration {
  schemaSpecifier: string;
  schemaExport: string;
  rendererSpecifier: string;
  rendererExport: string;
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
      fileName: "software-map.ts";
      collect(path: string, repo: string): Promise<Captured[]>;
    };

export interface ReviewSurfaceDescriptor<Kind extends string, Captured, Compiled> {
  kind: Kind;
  source: SurfaceSource<Captured>;
  capturedSchema: z.ZodType<Captured>;
  compile(model: Captured, evidence: SourceEvidenceResolver): Promise<Compiled>;
  compiledSchema: z.ZodType<Compiled>;
  browser: BrowserRegistration;
}

export type AnySurfaceDescriptor = ReviewSurfaceDescriptor<string, any, any>;
