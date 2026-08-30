import { createRoot } from "react-dom/client";
import type { ComponentType } from "react";
import { compiledDocumentSchema, type SourceRange } from "./diagram-family";

import "@xyflow/react/dist/style.css";
import "./review-styles.css";
import "./viewer.css";

export const CODE_DIAGRAM_OPEN_SOURCE_EVENT = "code-diagram:open-source";

export interface CodeDiagramOpenSourceDetail {
  sources: readonly SourceRange[];
  title?: string;
}

export type BrowserSurfaceRegistration = {
  schema: { parse(value: unknown): unknown };
  Renderer: ComponentType<{
    model: any;
    openEvidence: (source: SourceRange | readonly SourceRange[], title?: string) => void;
  }>;
};

export function mountReviewDocument(
  raw: unknown,
  registrations: Record<string, BrowserSurfaceRegistration>,
) {
  const documentModel = compiledDocumentSchema.parse(raw);
  for (const [index, surface] of documentModel.diagrams.entries()) {
    const registration = registrations[surface.kind];
    if (!registration)
      throw new Error(`Unknown code-diagram surface ${surface.kind} at index ${index}`);
    const roots = document.querySelectorAll<HTMLElement>(
      `[data-code-diagram-kind="${surface.kind}"][data-code-diagram-index="${index}"]`,
    );
    if (roots.length !== 1)
      throw new Error(
        `Expected one ${surface.kind} placeholder at index ${index}, found ${roots.length}`,
      );
    const model = registration.schema.parse(surface.model);
    createRoot(roots[0]!).render(<registration.Renderer model={model} openEvidence={openSource} />);
  }
}

function openSource(source: SourceRange | readonly SourceRange[], title?: string) {
  const sources = Array.isArray(source) ? source : [source];
  if (!sources.length) return;
  window.dispatchEvent(
    new CustomEvent<CodeDiagramOpenSourceDetail>(CODE_DIAGRAM_OPEN_SOURCE_EVENT, {
      detail: { sources, title },
    }),
  );
}
