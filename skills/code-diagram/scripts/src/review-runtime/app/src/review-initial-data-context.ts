export interface ReviewInitialData {
  softwareMapResolvedData: Array<{ key: string; response: unknown }>;
}

export function useReviewInitialData(): ReviewInitialData | null {
  return null;
}
