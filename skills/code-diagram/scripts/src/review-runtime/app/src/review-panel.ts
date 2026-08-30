import { useOfflineReviewRuntime } from "./offline-context";

interface ReviewPanelState {
  motion: "idle" | "restored";
  openPeek(anchor: any, _peek?: unknown): void;
}

export function useReviewPanel<T>(selector: (state: ReviewPanelState) => T): T {
  const runtime = useOfflineReviewRuntime();
  return selector({
    motion: "idle",
    openPeek(anchor) {
      const source = anchor?.peek?.resolution?.source ?? runtime.sourcesByAnchorId[anchor?.id];
      if (source) runtime.openEvidence(source, anchor.title);
    },
  });
}
