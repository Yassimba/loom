import { createRoot } from "react-dom/client";
import type { ComponentType } from "react";
import { browserDocumentSchema, type DiagramRendererProps, type SourceRange } from "../document/model";

import "@xyflow/react/dist/style.css";
import "./review.css";
import "./base.css";
import "./theme.css";

export const CODE_DIAGRAM_OPEN_SOURCE_EVENT = "code-diagram:open-source";

export interface CodeDiagramOpenSourceDetail {
  sources: readonly SourceRange[];
  title?: string;
}

export type BrowserSurfaceRegistration = {
  schema: { parse(value: unknown): unknown };
  Renderer: ComponentType<DiagramRendererProps<any>>;
};

export function mountReviewDocument(
  raw: unknown,
  registrations: Record<string, BrowserSurfaceRegistration>,
) {
  const documentModel = browserDocumentSchema.parse(raw);
  const roots = document.querySelectorAll<HTMLElement>(
    "[data-code-diagram-kind][data-code-diagram-index]",
  );
  if (roots.length !== documentModel.diagrams.length)
    throw new Error(
      `Expected ${documentModel.diagrams.length} placeholders, found ${roots.length}`,
    );
  for (const [index, surface] of documentModel.diagrams.entries()) {
    const registration = registrations[surface.kind];
    if (!registration)
      throw new Error(`Unknown code-diagram surface ${surface.kind} at index ${index}`);
    const root = roots[index]!;
    if (
      root.dataset.codeDiagramKind !== surface.kind ||
      root.dataset.codeDiagramIndex !== String(index)
    )
      throw new Error(
        `Expected ${surface.kind} placeholder at index ${index}`,
      );
    const model = registration.schema.parse(surface.model);
    createRoot(root).render(<registration.Renderer model={model} openEvidence={openSource} />);
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
