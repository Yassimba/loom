export const CANVAS_RELATIONSHIP_SEMANTIC_KINDS = [
  "dependency",
  "http",
  "async",
  "return",
  "optional",
  "primary",
  "forbidden",
  "published",
  "foreign-key",
] as const;

export type CanvasRelationshipSemanticKind =
  (typeof CANVAS_RELATIONSHIP_SEMANTIC_KINDS)[number];

export function isCanvasRelationshipSemanticKind(
  value: unknown,
): value is CanvasRelationshipSemanticKind {
  return CANVAS_RELATIONSHIP_SEMANTIC_KINDS.some((kind) => kind === value);
}
