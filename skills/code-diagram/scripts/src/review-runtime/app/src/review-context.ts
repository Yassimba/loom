export interface CommentDraftPlacement {
  target?: unknown;
  title?: string;
  body?: string;
  x?: number;
  y?: number;
  placement?: unknown;
  [key: string]: unknown;
}

export function useReview() {
  return {
    openCommentDraft: (_placement: CommentDraftPlacement) => undefined,
  };
}
