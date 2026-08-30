import type { CallStackEntry } from "./authoring";
import { callStackEntryAnchor } from "./authoring";
import type { SourceRange } from "./diagram-family";
import { CallStackDiff } from "./review-runtime/app/src/call-stack-diff";
import { OfflineReviewRuntime } from "./review-runtime/app/src/offline-context";

interface CompiledCallStack {
  title?: string;
  rows: Array<{
    entry: CallStackEntry;
    change: "added" | "removed" | "unchanged";
    depth: number;
    source: SourceRange;
  }>;
}

export function CallStackDiffRenderer({
  model,
  openEvidence,
}: {
  model: CompiledCallStack;
  openEvidence: (source: SourceRange | readonly SourceRange[], title?: string) => void;
}) {
  const base = model.rows
    .filter((row) => row.change !== "added")
    .map((row) => row.entry);
  const head = model.rows
    .filter((row) => row.change !== "removed")
    .map((row) => row.entry);
  const sourcesByAnchorId = Object.fromEntries(
    model.rows.map((row) => [callStackEntryAnchor(row.entry).id, row.source]),
  );
  return (
    <OfflineReviewRuntime openEvidence={openEvidence} sourcesByAnchorId={sourcesByAnchorId}>
      <CallStackDiff title={model.title} base={base} head={head} />
    </OfflineReviewRuntime>
  );
}
