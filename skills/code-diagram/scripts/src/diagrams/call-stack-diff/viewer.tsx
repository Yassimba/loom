import {
  callStackEntryAnchor,
  isCallsAssertion,
  type CallStackEntry,
} from "./authoring";
import type { DiagramRendererProps } from "../../document/model";
import {
  callStackBrowserSchema,
  callStackConnectorPrefix,
} from "./model";

export function CallStackDiffRenderer({
  model,
  openEvidence,
}: DiagramRendererProps<ReturnType<typeof callStackBrowserSchema.parse>>) {
  const added = model.rows.filter((row) => row.change === "added").length;
  const removed = model.rows.filter((row) => row.change === "removed").length;
  return (
    <div
      className="call-stack-diff diagram-design-tree"
      data-diagram-design-type="tree"
      data-review-call-stack="ready"
    >
      <div className="call-stack-hunk">
        <span className="call-stack-hunk-label">
          {model.title ? `@@ ${model.title} · base → head @@` : "@@ base → head @@"}
        </span>
        <span className="call-stack-hunk-counts">
          {added > 0 ? <span className="call-stack-count-added">+{added}</span> : null}
          {removed > 0 ? <span className="call-stack-count-removed">−{removed}</span> : null}
        </span>
      </div>
      <div className="call-stack-body" role="list">
        {model.rows.map((row, index) => {
          const anchor = callStackEntryAnchor(row.entry);
          const marker = row.change === "added" ? "+" : row.change === "removed" ? "-" : " ";
          return (
            <button
              key={`${anchor.id}-${index}`}
              type="button"
              role="listitem"
              className={`call-stack-row call-stack-${row.change}`}
              data-review-anchor-id={anchor.id}
              title={`${rowTooltip(row.entry)} — ${anchor.peek.props.file}:${anchor.peek.props.fromLine}`}
              onClick={() => openEvidence(row.source, anchor.title)}
            >
              <span className="call-stack-gutter">{marker}</span>
              <span className="call-stack-tree">{callStackConnectorPrefix(model.rows, index)}</span>
              <span className="call-stack-name">{anchor.title}</span>
              {isCallsAssertion(row.entry) ? (
                <span className="call-stack-asserted">≈ {row.entry.reason ?? "asserted"}</span>
              ) : null}
              <span className="call-stack-spacer" />
              <span className="call-stack-loc">
                {`${anchor.peek.props.file.split("/").pop()}:${anchor.peek.props.fromLine}`}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}

function rowTooltip(entry: CallStackEntry): string {
  if (!isCallsAssertion(entry)) return entry.title;
  const reason = entry.reason ? `: ${entry.reason}` : "";
  return `${entry.parent.title} → ${entry.child.title}${reason}`;
}

export { CallStackDiffRenderer as Renderer, callStackBrowserSchema as schema };
