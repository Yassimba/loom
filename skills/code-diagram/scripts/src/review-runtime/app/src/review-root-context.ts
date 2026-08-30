export function useReviewContainer(): HTMLElement | null {
  if (typeof document === "undefined") return null;
  return document.querySelector<HTMLElement>(".review-canvas-root") ?? document.body;
}

export function useReviewRoots() {
  const container = useReviewContainer();
  return { container, root: container };
}
