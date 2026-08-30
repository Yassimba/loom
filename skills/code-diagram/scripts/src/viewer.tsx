// Sequence renderer ported from devdotfast/review@8267620:
// packages/progressive-review/app/src/diagrams.tsx.
import {
  BaseEdge,
  EdgeLabelRenderer,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  getStraightPath,
  type Edge as ReactFlowEdge,
  type EdgeProps,
  type Node as ReactFlowNode,
  type NodeProps,
} from "@xyflow/react";
import { createRoot } from "react-dom/client";
import {
  type CSSProperties,
  type KeyboardEvent as ReactKeyboardEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";

import "@xyflow/react/dist/style.css";
import "./review-styles.css";
import "./viewer.css";

type SourceLine = { number: number; text: string };
type SourceRange = { file: string; fromLine: number; toLine: number; lines: SourceLine[] };
type InlineCode = { language?: string; text: string };
type Actor = { id: string; label: string };
type Message = {
  id: string;
  from: string;
  to: string;
  label: string;
  source?: SourceRange;
  code?: InlineCode;
};
type SequenceModel = {
  type: "sequence";
  id: string;
  label: string;
  actors: Record<string, Actor>;
  messages: Message[];
};
type DocumentModel = {
  version: 1;
  title: string;
  intro: string[];
  repo: string;
  revision: string | null;
  diagrams: SequenceModel[];
};
type SequenceMessage = Message & { fromActor: Actor; toActor: Actor };
type Sequence = {
  id: string;
  label: string;
  participants: Actor[];
  messages: SequenceMessage[];
};

declare global {
  var __CODE_DIAGRAM_DOCUMENT__: DocumentModel;
}

const sequenceNodeTypes = { sequenceParticipant: SequenceParticipantNode };
const sequenceEdgeTypes = { sequenceMessage: SequenceMessageEdge };

function App({ document }: { document: DocumentModel }) {
  return (
    <main className="review-canvas-root code-diagram-page">
      <article className="review-document code-diagram-document">
        <header className="code-diagram-document-header">
          <span>CODE DIAGRAM · REVIEW PARITY PREVIEW</span>
          <h1>{document.title}</h1>
          <p className="code-diagram-revision">
            {document.repo}
            {document.revision ? ` · ${document.revision.slice(0, 12)}` : ""}
          </p>
        </header>
        {document.intro.map((paragraph, index) => (
          <p key={`${index}-${paragraph}`}>{paragraph}</p>
        ))}
        {document.diagrams.map((diagram) => (
          <SequenceDiagram key={diagram.id} input={diagram} />
        ))}
      </article>
    </main>
  );
}

function createSequence(input: SequenceModel): Sequence {
  const messages = input.messages.map((message) => ({
    ...message,
    fromActor: input.actors[message.from],
    toActor: input.actors[message.to],
  }));
  return {
    id: input.id,
    label: input.label,
    participants: participantsForMessages(messages),
    messages,
  };
}

// Verbatim ordering algorithm from Review. Sources remain before targets where
// the graph is acyclic; cycles fall back to authored first-seen order.
function participantsForMessages(messages: SequenceMessage[]): Actor[] {
  const participants = new Map<string, { actor: Actor; order: number }>();
  const outgoing = new Map<string, Set<string>>();
  const incomingCount = new Map<string, number>();
  for (const message of messages) {
    if (!participants.has(message.fromActor.id)) {
      participants.set(message.fromActor.id, { actor: message.fromActor, order: participants.size });
      incomingCount.set(message.fromActor.id, 0);
    }
    if (!participants.has(message.toActor.id)) {
      participants.set(message.toActor.id, { actor: message.toActor, order: participants.size });
      incomingCount.set(message.toActor.id, 0);
    }
    if (message.fromActor.id === message.toActor.id) continue;
    const targets = outgoing.get(message.fromActor.id) ?? new Set<string>();
    if (!targets.has(message.toActor.id)) {
      targets.add(message.toActor.id);
      outgoing.set(message.fromActor.id, targets);
      incomingCount.set(message.toActor.id, (incomingCount.get(message.toActor.id) ?? 0) + 1);
    }
  }
  const byFirstSeen = (left: string, right: string) =>
    (participants.get(left)?.order ?? 0) - (participants.get(right)?.order ?? 0);
  const ready = [...participants.keys()]
    .filter((id) => (incomingCount.get(id) ?? 0) === 0)
    .sort(byFirstSeen);
  const ordered: Actor[] = [];
  const consumed = new Set<string>();
  while (ready.length > 0) {
    const id = ready.shift()!;
    if (consumed.has(id)) continue;
    consumed.add(id);
    const actor = participants.get(id)?.actor;
    if (actor) ordered.push(actor);
    for (const target of outgoing.get(id) ?? []) {
      incomingCount.set(target, (incomingCount.get(target) ?? 0) - 1);
      if ((incomingCount.get(target) ?? 0) === 0) {
        ready.push(target);
        ready.sort(byFirstSeen);
      }
    }
  }
  for (const [id, participant] of [...participants.entries()].sort(
    (left, right) => left[1].order - right[1].order,
  )) {
    if (!consumed.has(id)) ordered.push(participant.actor);
  }
  return ordered;
}

function SequenceDiagram({ input }: { input: SequenceModel }) {
  const sequence = useMemo(() => createSequence(input), [input]);
  const [tourIndex, setTourIndex] = useState<number | null>(null);
  const closeTour = useCallback(() => setTourIndex(null), []);
  useEffect(() => {
    if (tourIndex === null) return;
    const close = (event: KeyboardEvent) => {
      if (event.key === "Escape") closeTour();
    };
    window.addEventListener("keydown", close);
    return () => window.removeEventListener("keydown", close);
  }, [closeTour, tourIndex]);
  const openMessage = (messageId?: string) => {
    const index = messageId
      ? sequence.messages.findIndex((message) => message.id === messageId)
      : 0;
    setTourIndex(index < 0 ? 0 : index);
  };
  const active = tourIndex === null ? null : sequence.messages[tourIndex] ?? sequence.messages[0];
  return (
    <>
      <SequenceDiagramFigure sequence={sequence} activeMessageId={null} openMessage={openMessage} />
      {active ? (
        <div className="diagram-tour-overlay code-diagram-tour" role="dialog" aria-modal="true" aria-label={`${sequence.label} tour`}>
          <div className="diagram-tour-stage">
            <SequenceDiagramFigure
              sequence={sequence}
              activeMessageId={active.id}
              openMessage={openMessage}
              tourMode
            />
          </div>
          <div className="code-diagram-tour-separator" />
          <SourcePanel
            sequence={sequence}
            index={sequence.messages.indexOf(active)}
            onIndexChange={setTourIndex}
            onClose={closeTour}
          />
        </div>
      ) : null}
    </>
  );
}

function SequenceDiagramFigure({
  sequence,
  activeMessageId,
  openMessage,
  tourMode = false,
}: {
  sequence: Sequence;
  activeMessageId: string | null;
  openMessage: (messageId?: string) => void;
  tourMode?: boolean;
}) {
  const sequenceScrollRef = useRef<HTMLDivElement | null>(null);
  const [availableWidth, setAvailableWidth] = useState(0);
  useEffect(() => {
    const scroll = sequenceScrollRef.current;
    if (!scroll) return;
    const update = () => setAvailableWidth(scroll.clientWidth);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(scroll);
    return () => observer.disconnect();
  }, []);
  const laneWidth = Math.max(176, Math.floor(availableWidth / Math.max(1, sequence.participants.length)));
  const messageTop = 112;
  const messageGap = 76;
  const width = Math.max(320, sequence.participants.length * laneWidth);
  const height = messageTop + sequence.messages.length * messageGap + 42;
  const nodes: ReactFlowNode[] = useMemo(
    () =>
      sequence.participants.map((participant, index) => ({
        id: participant.id,
        type: "sequenceParticipant",
        position: { x: index * laneWidth, y: 0 },
        width: laneWidth,
        height,
        data: { participant, height, messages: sequence.messages, messageGap, messageTop },
        draggable: false,
        selectable: false,
      })),
    [height, laneWidth, sequence],
  );
  const edges: ReactFlowEdge[] = useMemo(
    () =>
      sequence.messages.map((message, index) => {
        const active = activeMessageId === message.id;
        const color = active ? "var(--accent)" : "var(--edge-muted)";
        return {
          id: message.id,
          type: "sequenceMessage",
          source: message.fromActor.id,
          target: message.toActor.id,
          sourceHandle: sequenceHandleId(message.id, "source"),
          targetHandle: sequenceHandleId(message.id, "target"),
          markerEnd: { type: MarkerType.ArrowClosed, color },
          style: { stroke: color },
          data: { message, width, active, openMessage, stepNumber: tourMode ? index + 1 : null },
          className: active ? "sequence-message clickable active" : "sequence-message clickable",
          zIndex: active ? 2 : 1,
        };
      }),
    [activeMessageId, openMessage, sequence, tourMode, width],
  );
  useEffect(() => {
    const scroll = sequenceScrollRef.current;
    if (!scroll || !activeMessageId) return;
    const left = sequenceActiveMessageScrollTarget({
      sequence,
      activeMessageId,
      laneWidth,
      viewportWidth: scroll.clientWidth,
      scrollWidth: scroll.scrollWidth,
      currentScrollLeft: scroll.scrollLeft,
    });
    const top = sequenceActiveMessageScrollTopTarget({
      sequence,
      activeMessageId,
      messageTop,
      messageGap,
      viewportHeight: scroll.clientHeight,
      scrollHeight: scroll.scrollHeight,
      currentScrollTop: scroll.scrollTop,
    });
    if (left !== null || top !== null) scroll.scrollTo({ left: left ?? undefined, top: top ?? undefined, behavior: "smooth" });
  }, [activeMessageId, laneWidth, sequence]);
  const style = {
    "--sequence-width": `${width}px`,
    "--sequence-height": `${height}px`,
    "--sequence-lane-width": `${laneWidth}px`,
  } as CSSProperties;
  return (
    <figure
      className={activeMessageId ? "sequence-diagram sequence-tour sequence-tour--active" : "sequence-diagram sequence-tour"}
      style={style}
      data-sequence-tour-id={sequence.id}
    >
      <figcaption className="diagram-header">
        <div className="diagram-header-main">
          <span className="diagram-kind-badge">SEQ</span>
          <span className="diagram-header-title">{sequence.label}</span>
          <em className="diagram-header-meta">{sequence.messages.length} stops</em>
        </div>
        {!tourMode ? (
          <button type="button" className="diagram-tour-button" onClick={() => openMessage()}>
            Tour
          </button>
        ) : null}
      </figcaption>
      <div ref={sequenceScrollRef} className="sequence-diagram-body">
        <ReactFlow
          id={`${sequence.id}-${tourMode ? "tour" : "inline"}`}
          colorMode="dark"
          nodes={nodes}
          edges={edges}
          nodeTypes={sequenceNodeTypes}
          edgeTypes={sequenceEdgeTypes}
          onEdgeClick={(event, edge) => {
            event.stopPropagation();
            openMessage(edge.id);
          }}
          defaultViewport={{ x: 0, y: 0, zoom: 1 }}
          nodesDraggable={false}
          nodesConnectable={false}
          elementsSelectable={false}
          panActivationKeyCode={null}
          panOnDrag={false}
          preventScrolling={false}
          zoomOnScroll={false}
          zoomOnPinch={false}
          zoomOnDoubleClick={false}
          proOptions={{ hideAttribution: true }}
        />
      </div>
    </figure>
  );
}

function SequenceParticipantNode({ data }: NodeProps) {
  const { participant, height, messages, messageGap, messageTop } = data as unknown as {
    participant: Actor;
    height: number;
    messages: SequenceMessage[];
    messageGap: number;
    messageTop: number;
  };
  const activeMessages = messages.filter(
    (message) => message.fromActor.id === participant.id || message.toActor.id === participant.id,
  );
  return (
    <div className="sequence-participant-node" style={{ height }} data-sequence-actor-id={participant.id}>
      <div className="sequence-participant-comment-target">
        <span className="sequence-participant-label" title={participant.label}>
          {participant.label}
        </span>
      </div>
      <div className="sequence-lifeline" />
      {activeMessages.flatMap((message, index) => {
        const messageIndex = messages.findIndex((item) => item.id === message.id);
        const top = messageTop + messageIndex * messageGap;
        const handles: Array<{ id: string; type: "source" | "target" }> = [];
        if (message.fromActor.id === participant.id) handles.push({ id: sequenceHandleId(message.id, "source"), type: "source" });
        if (message.toActor.id === participant.id) handles.push({ id: sequenceHandleId(message.id, "target"), type: "target" });
        return handles.map((handle) => (
          <Handle
            key={`${message.id}-${handle.type}-${index}`}
            id={handle.id}
            type={handle.type}
            position={Position.Right}
            className="sequence-message-handle"
            style={{ left: "50%", top: sequenceMessageHandleTop(message, handle.type, top) }}
          />
        ));
      })}
    </div>
  );
}

function SequenceMessageEdge(props: EdgeProps) {
  const [hovering, setHovering] = useState(false);
  const data = props.data as {
    message: SequenceMessage;
    width: number;
    active: boolean;
    openMessage: (id?: string) => void;
    stepNumber: number | null;
  };
  const self = props.source === props.target;
  const loopDirection = props.sourceX + 72 > data.width ? -1 : 1;
  const loopX = props.sourceX + loopDirection * 54;
  const edgePath = self
    ? sequenceSelfMessagePath({
        sourceX: props.sourceX,
        sourceY: props.sourceY,
        targetX: props.targetX,
        targetY: props.targetY,
        width: data.width,
      })
    : getStraightPath({
        sourceX: props.sourceX,
        sourceY: props.sourceY,
        targetX: props.targetX,
        targetY: props.targetY,
      })[0];
  const labelX = self ? (props.sourceX + loopX) / 2 : (props.sourceX + props.targetX) / 2;
  const labelY = props.sourceY - 12;
  const activate = () => data.openMessage(data.message.id);
  const onKeyDown = (event: ReactKeyboardEvent) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    activate();
  };
  return (
    <>
      <BaseEdge
        id={props.id}
        path={edgePath}
        markerEnd={props.markerEnd}
        className={data.active ? "sequence-message clickable active" : "sequence-message clickable"}
        style={props.style}
      />
      <path
        d={edgePath}
        className="sequence-message-hit-area"
        onMouseEnter={() => setHovering(true)}
        onMouseLeave={() => setHovering(false)}
        onClick={activate}
      />
      <EdgeLabelRenderer>
        <button
          type="button"
          className={`sequence-message-dot${data.active ? " active" : ""}${data.stepNumber !== null ? " sequence-message-dot--step" : ""}`}
          style={{ transform: `translate(-50%, -50%) translate(${props.sourceX}px,${props.sourceY}px)` }}
          data-review-anchor-id={data.message.id}
          onClick={activate}
          aria-label={data.message.label}
        >
          {data.stepNumber}
        </button>
        <div
          className={hovering ? "sequence-message-comment-target comment-target-hovered" : "sequence-message-comment-target"}
          style={{ transform: `translate(-50%, -100%) translate(${labelX}px,${labelY}px)` }}
        >
          <span
            role="button"
            tabIndex={0}
            className={data.active ? "sequence-message-label clickable active" : "sequence-message-label clickable"}
            data-review-anchor-id={data.message.id}
            onMouseEnter={() => setHovering(true)}
            onMouseLeave={() => setHovering(false)}
            onClick={activate}
            onKeyDown={onKeyDown}
          >
            {data.message.label}
          </span>
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

function SourcePanel({
  sequence,
  index,
  onIndexChange,
  onClose,
}: {
  sequence: Sequence;
  index: number;
  onIndexChange: (index: number) => void;
  onClose: () => void;
}) {
  const message = sequence.messages[index];
  const source = message.source;
  const code = message.code;
  return (
    <aside className="code-diagram-source-panel" aria-label="Source evidence">
      <header>
        <div>
          <span>{index + 1} / {sequence.messages.length}</span>
          <h2>{message.label}</h2>
          <p>{message.fromActor.label} → {message.toActor.label}</p>
        </div>
        <button type="button" onClick={onClose} aria-label="Close source evidence">×</button>
      </header>
      {source ? (
        <>
          <div className="code-diagram-source-path">{source.file}:{source.fromLine}-{source.toLine}</div>
          <pre className="code-diagram-source"><code>{source.lines.map((line) => (
            <span className="code-diagram-source-line" key={line.number}>
              <span className="code-diagram-line-number">{line.number}</span>
              <span>{line.text || " "}</span>
            </span>
          ))}</code></pre>
        </>
      ) : (
        <>
          <div className="code-diagram-source-path">{code?.language ?? "inline code"}</div>
          <pre className="code-diagram-source"><code>{code?.text}</code></pre>
        </>
      )}
      <nav aria-label="Tour controls">
        <button type="button" disabled={index === 0} onClick={() => onIndexChange(index - 1)}>Previous</button>
        <button type="button" disabled={index === sequence.messages.length - 1} onClick={() => onIndexChange(index + 1)}>Next</button>
      </nav>
    </aside>
  );
}

function sequenceMessageHandleTop(
  message: SequenceMessage,
  handleType: "source" | "target",
  messageTop: number,
): number {
  return message.fromActor.id === message.toActor.id && handleType === "target" ? messageTop + 24 : messageTop;
}

function sequenceSelfMessagePath(input: {
  sourceX: number;
  sourceY: number;
  targetX: number;
  targetY: number;
  width: number;
}): string {
  const loopDirection = input.sourceX + 72 > input.width ? -1 : 1;
  const loopX = input.sourceX + loopDirection * 54;
  return `M ${input.sourceX} ${input.sourceY} H ${loopX} V ${input.targetY} H ${input.targetX}`;
}

function sequenceActiveMessageScrollTarget(input: {
  sequence: Sequence;
  activeMessageId: string;
  laneWidth: number;
  viewportWidth: number;
  scrollWidth: number;
  currentScrollLeft: number;
  padding?: number;
}) {
  const { sequence, activeMessageId, laneWidth, viewportWidth, scrollWidth, currentScrollLeft, padding = 24 } = input;
  const maxScrollLeft = scrollWidth - viewportWidth;
  if (maxScrollLeft <= 0 || viewportWidth <= 0) return null;
  const message = sequence.messages.find((item) => item.id === activeMessageId);
  if (!message) return null;
  const fromIndex = sequence.participants.findIndex((actor) => actor.id === message.fromActor.id);
  const toIndex = sequence.participants.findIndex((actor) => actor.id === message.toActor.id);
  if (fromIndex < 0 || toIndex < 0) return null;
  const targetLeft = Math.max(0, Math.min(fromIndex, toIndex) * laneWidth - padding);
  const targetRight = Math.min(scrollWidth, (Math.max(fromIndex, toIndex) + 1) * laneWidth + padding);
  const visibleRight = currentScrollLeft + viewportWidth;
  const clamp = (left: number) => Math.min(Math.max(left, 0), maxScrollLeft);
  if (targetLeft >= currentScrollLeft && targetRight <= visibleRight) return currentScrollLeft;
  if (targetRight - targetLeft > viewportWidth || targetLeft < currentScrollLeft) return clamp(targetLeft);
  return clamp(targetRight - viewportWidth);
}

function sequenceActiveMessageScrollTopTarget(input: {
  sequence: Sequence;
  activeMessageId: string;
  messageTop: number;
  messageGap: number;
  viewportHeight: number;
  scrollHeight: number;
  currentScrollTop: number;
  padding?: number;
}) {
  const { sequence, activeMessageId, messageTop, messageGap, viewportHeight, scrollHeight, currentScrollTop, padding = 24 } = input;
  const maxScrollTop = scrollHeight - viewportHeight;
  if (maxScrollTop <= 0 || viewportHeight <= 0) return null;
  const index = sequence.messages.findIndex((message) => message.id === activeMessageId);
  if (index < 0) return null;
  const rowTop = messageTop + index * messageGap;
  const targetTop = Math.max(0, rowTop - padding);
  const targetBottom = Math.min(scrollHeight, rowTop + messageGap + padding);
  const visibleBottom = currentScrollTop + viewportHeight;
  const clamp = (top: number) => Math.min(Math.max(top, 0), maxScrollTop);
  if (targetTop >= currentScrollTop && targetBottom <= visibleBottom) return currentScrollTop;
  if (targetBottom - targetTop > viewportHeight || targetTop < currentScrollTop) return clamp(targetTop);
  return clamp(targetBottom - viewportHeight);
}

function sequenceHandleId(messageId: string, type: "source" | "target") {
  return `${type}-${messageId}`;
}

const root = document.getElementById("code-diagram-root");
if (!root) throw new Error("code-diagram root is missing");
createRoot(root).render(<App document={globalThis.__CODE_DIAGRAM_DOCUMENT__} />);
