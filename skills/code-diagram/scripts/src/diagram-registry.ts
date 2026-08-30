import React, { type ReactElement } from "react";

import {
  callStackDiffPropsSchema,
  callStackEntryAnchor,
  isCallsAssertion,
  sequenceDiagramPropsSchema,
  type CallStackDiffProps,
  type CallStackEntry,
  type SequenceDiagramProps,
} from "./authoring";
import { callStackEvidenceErrors, diffCallStacks } from "./call-stack-diff";
import {
  createDatabaseLensComponents,
  capturedDatabaseLensSchema,
  compiledDatabaseLensSchema,
  compileDatabaseLens,
} from "./database-lens-model";
import type {
  AnySurfaceDescriptor,
  CompiledSurface,
  SourceEvidenceResolver,
} from "./diagram-family";
import { createSequence } from "./sequence-model";
import { collectSoftwareMap, compileSoftwareMap } from "./software-map-surface";
import { compiledSoftwareMapSchema, serializedSoftwareMapSchema } from "./software-map-schema";
import { callStackBrowserSchema } from "./surface-schemas";

const compiledCallStackSchema = callStackBrowserSchema;

export function createSurfaceRegistry(modelSource: string): AnySurfaceDescriptor[] {
  const descriptors: AnySurfaceDescriptor[] = [
    {
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
      async compile(model: SequenceDiagramProps, evidence: SourceEvidenceResolver) {
        return sequenceDiagramPropsSchema.parse({
          ...model,
          messages: await Promise.all(
            model.messages.map(async (message) =>
              !message.anchor?.peek
                ? message
                : {
                    ...message,
                    anchor: {
                      ...message.anchor,
                      peek: {
                        ...message.anchor.peek,
                        resolution: {
                          source: await evidence.resolveRange(message.anchor.peek.props),
                        },
                      },
                    },
                  },
            ),
          ),
        });
      },
      compiledSchema: sequenceDiagramPropsSchema,
      browser: {
        schemaSpecifier: "./src/surface-schemas.ts",
        schemaExport: "sequenceBrowserSchema",
        rendererSpecifier: "./src/sequence-viewer.tsx",
        rendererExport: "SequenceDiagramRenderer",
      },
    },
    {
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
      async compile(model: CallStackDiffProps, evidence: SourceEvidenceResolver) {
        const rows = diffCallStacks(model.base, model.head);
        const errors = callStackEvidenceErrors(rows, (file, side) =>
          evidence.changedLines(file, side),
        );
        if (errors.length) throw new Error(`CALL_STACK_EVIDENCE_INVALID: ${errors.join("\n")}`);
        return compiledCallStackSchema.parse({
          title: model.title,
          rows: await Promise.all(
            rows.map(async (row) => {
              await resolveAllEntryEvidence(row.entry, evidence);
              const anchor = callStackEntryAnchor(row.entry);
              return { ...row, source: await evidence.resolveRange(anchor.peek.props) };
            }),
          ),
        });
      },
      compiledSchema: compiledCallStackSchema,
      browser: {
        schemaSpecifier: "./src/surface-schemas.ts",
        schemaExport: "callStackBrowserSchema",
        rendererSpecifier: "./src/call-stack-diff-viewer.tsx",
        rendererExport: "CallStackDiffRenderer",
      },
    },
    {
      kind: "database-lens",
      source: { type: "mdx", createComponents: createDatabaseLensComponents },
      capturedSchema: capturedDatabaseLensSchema,
      compile: compileDatabaseLens,
      compiledSchema: compiledDatabaseLensSchema,
      browser: {
        schemaSpecifier: "./src/database-lens-model.ts",
        schemaExport: "compiledDatabaseLensSchema",
        rendererSpecifier: "./src/database-lens-viewer.tsx",
        rendererExport: "DatabaseLensRenderer",
      },
    },
    {
      kind: "software-map",
      source: {
        type: "artifact",
        fileName: "software-map.ts",
        collect: (artifactPath, repo) => collectSoftwareMap(artifactPath, repo, modelSource),
      },
      capturedSchema: serializedSoftwareMapSchema,
      compile: compileSoftwareMap,
      compiledSchema: compiledSoftwareMapSchema,
      browser: {
        schemaSpecifier: "./src/software-map-schema.ts",
        schemaExport: "compiledSoftwareMapSchema",
        rendererSpecifier: "./src/software-map-viewer.tsx",
        rendererExport: "SoftwareMapRenderer",
      },
    },
  ];
  validateRegistry(descriptors);
  return descriptors;
}

export function validateRegistry(registry: AnySurfaceDescriptor[]) {
  const kinds = new Set<string>();
  const componentNames = new Set<string>();
  for (const descriptor of registry) {
    if (kinds.has(descriptor.kind)) throw new Error(`Duplicate surface kind ${descriptor.kind}`);
    kinds.add(descriptor.kind);
    for (const value of Object.values(descriptor.browser))
      if (!value) throw new Error(`Incomplete browser registration for ${descriptor.kind}`);
    if (descriptor.source.type === "mdx") {
      const components = descriptor.source.createComponents(() => React.createElement("div"));
      for (const name of Object.keys(components)) {
        if (componentNames.has(name)) throw new Error(`Duplicate MDX component ${name}`);
        componentNames.add(name);
      }
    }
  }
}

export function createMdxComponents(
  registry: AnySurfaceDescriptor[],
  emit: (kind: string, model: unknown) => ReactElement,
): Record<string, React.ComponentType<any>> {
  const components: Record<string, React.ComponentType<any>> = {};
  for (const descriptor of registry) {
    if (descriptor.source.type !== "mdx") continue;
    Object.assign(
      components,
      descriptor.source.createComponents((model) =>
        emit(descriptor.kind, descriptor.capturedSchema.parse(model)),
      ),
    );
  }
  return components;
}

export async function compileSurfaces(
  captured: CompiledSurface[],
  registry: AnySurfaceDescriptor[],
  evidence: SourceEvidenceResolver,
): Promise<CompiledSurface[]> {
  const byKind = new Map(registry.map((descriptor) => [descriptor.kind, descriptor]));
  return Promise.all(
    captured.map(async ({ kind, model }) => {
      const descriptor = byKind.get(kind);
      if (!descriptor) throw new Error(`Unknown surface kind ${kind}`);
      const compiled = await descriptor.compile(descriptor.capturedSchema.parse(model), evidence);
      return { kind, model: descriptor.compiledSchema.parse(compiled) };
    }),
  );
}

async function resolveAllEntryEvidence(entry: CallStackEntry, evidence: SourceEvidenceResolver) {
  if (isCallsAssertion(entry))
    await Promise.all([
      evidence.resolveRange(entry.parent.peek.props),
      evidence.resolveRange(entry.child.peek.props),
    ]);
  else await evidence.resolveRange(entry.peek.props);
}
