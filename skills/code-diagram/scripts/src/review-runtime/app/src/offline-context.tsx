import { createContext, useContext, type ReactNode } from "react";
import type { SourceRange } from "../../../diagram-family";

interface OfflineReviewRuntimeValue {
  openEvidence(source: SourceRange, title?: string): void;
  sourcesByAnchorId: Readonly<Record<string, SourceRange>>;
}

const OfflineReviewRuntimeContext = createContext<OfflineReviewRuntimeValue>({
  openEvidence: () => undefined,
  sourcesByAnchorId: {},
});

export function OfflineReviewRuntime({
  children,
  openEvidence,
  sourcesByAnchorId = {},
}: {
  children: ReactNode;
  openEvidence(source: SourceRange, title?: string): void;
  sourcesByAnchorId?: Readonly<Record<string, SourceRange>>;
}) {
  return (
    <OfflineReviewRuntimeContext.Provider value={{ openEvidence, sourcesByAnchorId }}>
      {children}
    </OfflineReviewRuntimeContext.Provider>
  );
}

export function useOfflineReviewRuntime() {
  return useContext(OfflineReviewRuntimeContext);
}
