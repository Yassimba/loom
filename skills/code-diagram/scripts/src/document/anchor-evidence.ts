import type { PeekableAnchorRef } from "../authoring/core";
import type { SourceEvidenceResolver, SourceRange } from "./model";

export type ResolvedAnchor<Anchor extends PeekableAnchorRef = PeekableAnchorRef> = Anchor & {
  peek: Anchor["peek"] & { resolution: { source: SourceRange } };
};

/** Resolve a peekable authoring anchor while preserving its diagram-specific fields. */
export async function resolveAnchorEvidence<Anchor extends PeekableAnchorRef>(
  anchor: Anchor,
  evidence: SourceEvidenceResolver,
): Promise<{ anchor: ResolvedAnchor<Anchor>; source: SourceRange }> {
  const source = await evidence.resolveRange(anchor.peek.props);
  return {
    anchor: {
      ...anchor,
      peek: { ...anchor.peek, resolution: { source } },
    } as ResolvedAnchor<Anchor>,
    source,
  };
}
