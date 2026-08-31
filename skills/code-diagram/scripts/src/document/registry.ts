import React, { type ReactElement } from "react";

import { callStackDiffSurfaceDescriptor } from "../diagrams/call-stack-diff/descriptor";
import { databaseLensSurfaceDescriptor } from "../diagrams/database-lens/descriptor";
import { sequenceSurfaceDescriptor } from "../diagrams/sequence/descriptor";
import { createSoftwareMapSurfaceDescriptor } from "../diagrams/software-map/descriptor";
import type {
  AnySurfaceDescriptor,
  CompiledSurface,
  SourceEvidenceResolver,
} from "./model";

export function createSurfaceRegistry(modelSource: string): AnySurfaceDescriptor[] {
  const descriptors: AnySurfaceDescriptor[] = [
    sequenceSurfaceDescriptor,
    callStackDiffSurfaceDescriptor,
    databaseLensSurfaceDescriptor,
    createSoftwareMapSurfaceDescriptor(modelSource),
  ];
  validateRegistry(descriptors);
  return descriptors;
}

export function validateRegistry(registry: readonly AnySurfaceDescriptor[]) {
  const kinds = new Set<string>();
  const componentNames = new Set<string>();
  for (const descriptor of registry) {
    if (kinds.has(descriptor.kind))
      throw new Error(`Duplicate surface kind ${descriptor.kind}`);
    kinds.add(descriptor.kind);
    if (descriptor.source.type === "mdx") {
      const components = descriptor.source.createComponents(() =>
        React.createElement("div"),
      );
      for (const name of Object.keys(components)) {
        if (componentNames.has(name))
          throw new Error(`Duplicate MDX component ${name}`);
        componentNames.add(name);
      }
    }
  }
}

export function createMdxComponents(
  registry: readonly AnySurfaceDescriptor[],
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
  captured: readonly CompiledSurface[],
  registry: readonly AnySurfaceDescriptor[],
  evidence: SourceEvidenceResolver,
): Promise<CompiledSurface[]> {
  return Promise.all(
    captured.map(async ({ kind, model }) => {
      const descriptor = registry.find((candidate) => candidate.kind === kind);
      if (!descriptor) throw new Error(`Unknown surface kind ${kind}`);
      const compiled = await descriptor.compile(model, evidence);
      return { kind, model: descriptor.compiledSchema.parse(compiled) };
    }),
  );
}
