import type { DiagramSnapshot } from "../../canvas/model";
import type { ChangeState } from "../../document/model";
import type { NormalizedSoftwareModel } from "./model";
import {
  type C4Projection,
  isInlineC4Expandable,
} from "./projection";

export interface SoftwareMapDiffCounts {
  additions: number;
  deletions: number;
}

export interface SoftwareMapChangeSummary extends SoftwareMapDiffCounts {
  changeStatus: ChangeState;
}

export function initialSoftwareMapExpandedNodeIds(
  model: NormalizedSoftwareModel,
): Set<string> {
  return new Set(
    model.elements
      .filter(
        (element) =>
          element.type !== "component" && isInlineC4Expandable(element),
      )
      .map((element) => element.path),
  );
}

export function buildSoftwareMapChangeSummaries(
  model: NormalizedSoftwareModel,
  diffCounts: Readonly<Record<string, SoftwareMapDiffCounts>>,
): ReadonlyMap<string, SoftwareMapChangeSummary> {
  const summaries = new Map<string, SoftwareMapChangeSummary>();

  const summarize = (path: string): SoftwareMapChangeSummary => {
    const cached = summaries.get(path);
    if (cached) return cached;

    const element = model.elementsByPath.get(path)!;
    const ownCounts = Object.hasOwn(diffCounts, path)
      ? diffCounts[path]!
      : { additions: 0, deletions: 0 };
    const hasOwnCoverage = Boolean(element.coverage);
    let additions =
      element.type === "codeElement"
        ? ownCounts.additions
        : 0;
    let deletions =
      element.type === "codeElement"
        ? ownCounts.deletions
        : 0;
    let hasChangedDescendant = false;

    for (const childPath of element.children) {
      const child = summarize(childPath);
      const childElement = model.elementsByPath.get(childPath)!;
      if (!hasOwnCoverage && childElement.type !== "codeElement") {
        additions += child.additions;
        deletions += child.deletions;
      }
      if (child.changeStatus !== "unchanged")
        hasChangedDescendant = true;
    }

    const summary: SoftwareMapChangeSummary = {
      changeStatus: inferSoftwareMapChangeStatus({
        authoredStatus: element.changeStatus,
        additions,
        deletions,
        hasChangedDescendant,
      }),
      additions,
      deletions,
    };
    summaries.set(path, summary);
    return summary;
  };

  for (const element of model.elements) summarize(element.path);
  return summaries;
}

function inferSoftwareMapChangeStatus({
  authoredStatus,
  additions,
  deletions,
  hasChangedDescendant,
}: {
  authoredStatus?: ChangeState;
  additions: number;
  deletions: number;
  hasChangedDescendant: boolean;
}): ChangeState {
  if (authoredStatus === "added" || authoredStatus === "removed")
    return authoredStatus;
  if (additions > 0 || deletions > 0) return "modified";
  if (authoredStatus === "modified") return authoredStatus;
  return hasChangedDescendant ? "modified" : "unchanged";
}

export function softwareMapSnapshotFromInlineC4Projection({
  projection,
  changeSummaries,
}: {
  projection: C4Projection;
  changeSummaries: ReadonlyMap<string, SoftwareMapChangeSummary>;
}): DiagramSnapshot {
  return {
    view: "inline-c4",
    selectedNodeId: projection.selectedNodeId ?? projection.nodes[0]?.id,
    nodes: projection.nodes.map((element) => {
      const summary = changeSummaries.get(element.path);
      return {
        id: element.id,
        label: element.label,
        type: element.type,
        path: element.path,
        description: element.description,
        changeStatus: summary?.changeStatus ?? element.changeStatus,
        dataStoreKind: element.dataStoreKind,
        additions: summary?.additions,
        deletions: summary?.deletions,
        parentId: element.parentPath ?? null,
        file: element.element?.sourceRanges?.[0]?.file,
        line: element.element?.sourceRanges?.[0]?.fromLine,
        boundary: element.external,
        expanded: element.isExpanded,
        expandable: element.isExpandable,
        childCount: element.childCount,
        dataStoreSchemaSections: element.dataStoreSchemaSections,
      };
    }),
    relationships: projection.relationships.map((relationship) => ({
      id: relationship.id,
      from: relationship.from,
      to: relationship.to,
      label: relationship.label,
      semanticKind: relationship.semanticKind,
      kind: relationship.kind,
      hideLabel: relationship.hideLabel,
      fromSchemaFieldPath: relationship.fromSchemaFieldPath,
      toSchemaFieldPath: relationship.toSchemaFieldPath,
      fromSchemaEndpointKind: relationship.fromSchemaEndpointKind,
      toSchemaEndpointKind: relationship.toSchemaEndpointKind,
    })),
  };
}
