import type { SourceRange } from "./diagram-family";
import type { SequenceDiagramProps } from "./authoring";
import { OfflineReviewRuntime } from "./review-runtime/app/src/offline-context";
import { SequenceDiagram } from "./review-runtime/app/src/diagrams";

export function SequenceDiagramRenderer({
  model,
  openEvidence,
}: {
  model: SequenceDiagramProps;
  openEvidence: (source: SourceRange | readonly SourceRange[], title?: string) => void;
}) {
  return (
    <OfflineReviewRuntime openEvidence={openEvidence}>
      <SequenceDiagram {...model} />
    </OfflineReviewRuntime>
  );
}
