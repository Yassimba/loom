export type ReviewTheme = "dark" | "light";
export type ReviewNodeTint = "none" | "slate" | "mineral";

export function useReviewDebugSettings() {
  return {
    showModifiedOnly: false,
    setShowModifiedOnly: (_value: boolean) => undefined,
    showRemovedNodes: true,
    setShowRemovedNodes: (_value: boolean) => undefined,
    theme: "dark" as const,
    nodeTint: "none" as const,
    setNodeTint: (_value: ReviewNodeTint) => undefined,
  };
}
