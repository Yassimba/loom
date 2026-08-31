import {
  BaseEdge,
  EdgeLabelRenderer,
  type EdgeMouseHandler,
  Handle,
  MarkerType,
  Position,
  ReactFlow,
  type Edge as ReactFlowEdge,
  type EdgeProps as ReactFlowEdgeProps,
  type Node as ReactFlowNode,
  type NodeProps as ReactFlowNodeProps,
  getStraightPath,
} from "@xyflow/react";
import {
  type CSSProperties,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { ActorRef, AnchorRef } from "../../authoring/core";
import {
  type SequenceDiagramProps,
  sequenceDiagramPropsSchema,
} from "./authoring";
import type { DiagramRendererProps } from "../../document/model";
import { hasTextSelectionWithin } from "../../viewer/text-selection";
import { createSequence, type SequenceMessage } from "./model";

const sequenceNodeTypes = { sequenceParticipant: SequenceParticipantNode };
const sequenceEdgeTypes = { sequenceMessage: SequenceMessageEdge };

export function SequenceDiagramRenderer({
  model,
  openEvidence,
}: DiagramRendererProps<SequenceDiagramProps>) {
  const sequence = useMemo(() => createSequence(model), [model.label, model.messages]);
  const openSource = (anchor: AnchorRef) => {
    const source = anchor.peek?.resolution?.source;
    if (source) openEvidence(source, anchor.title);
  };
  return <SequenceDiagramFigure sequence={sequence} openSource={openSource} />;
}

function SequenceDiagramFigure({
  sequence,
  openSource,
}: {
  sequence: ReturnType<typeof createSequence>;
  openSource: (anchor: AnchorRef) => void;
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
  // Lanes spread across the full diagram width; 176px is the floor below which
  // the body scrolls horizontally instead of compressing further.
  const laneWidth = Math.max(
    176,
    Math.floor(availableWidth / sequence.participants.length),
  );
  const messageTop = 112;
  const messageGap = 76;
  const width = Math.max(320, sequence.participants.length * laneWidth);
  const height = messageTop + sequence.messages.length * messageGap + 42;
  const reactFlowNodes: ReactFlowNode[] = useMemo(
    () =>
      sequence.participants.map((participant, index) => ({
        id: participant.id,
        type: "sequenceParticipant",
        position: { x: index * laneWidth, y: 0 },
        width: laneWidth,
        height,
        data: {
          participant,
          height,
          messages: sequence.messages,
          messageGap,
          messageTop,
        },
        draggable: false,
        selectable: false,
      })),
    [height, laneWidth, sequence],
  );
  const reactFlowEdges: ReactFlowEdge[] = useMemo(
    () =>
      sequence.messages.map((message) => {
        return {
          id: message.id,
          type: "sequenceMessage",
          source: message.from.id,
          target: message.to.id,
          sourceHandle: sequenceHandleId(message.id, "source"),
          targetHandle: sequenceHandleId(message.id, "target"),
          markerEnd: {
            type: MarkerType.ArrowClosed,
            color: "var(--edge-muted)",
          },
          style: { stroke: "var(--edge-muted)" },
          data: { message, width, openSource },
          className: "sequence-message clickable",
          zIndex: 1,
        };
      }),
    [openSource, sequence, width],
  );
  const onEdgeClick: EdgeMouseHandler = (event, edge) => {
    event.stopPropagation();
    const data = edge.data as { message?: SequenceMessage } | undefined;
    if (data?.message) openSource(data.message.anchor);
  };
  const scrollSequenceHorizontally = useCallback((event: WheelEvent) => {
    const scroll = event.currentTarget;
    if (
      !(scroll instanceof HTMLElement) ||
      !scrollDiagramHorizontally(scroll, event)
    ) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
  }, []);
  useEffect(() => {
    const scroll = sequenceScrollRef.current;
    if (!scroll) return;
    scroll.addEventListener("wheel", scrollSequenceHorizontally, {
      capture: true,
      passive: false,
    });
    return () =>
      scroll.removeEventListener("wheel", scrollSequenceHorizontally, {
        capture: true,
      });
  }, [scrollSequenceHorizontally]);
  const style = {
    "--sequence-width": `${width}px`,
    "--sequence-height": `${height}px`,
    "--sequence-lane-width": `${laneWidth}px`,
  } as CSSProperties;

  return (
    <figure
      className="sequence-diagram diagram-design-sequence"
      data-diagram-design-type="sequence"
      style={style}
      tabIndex={-1}
    >
      <div ref={sequenceScrollRef} className="sequence-diagram-body">
        <ReactFlow
            colorMode="dark"
            nodes={reactFlowNodes}
            edges={reactFlowEdges}
            nodeTypes={sequenceNodeTypes}
            edgeTypes={sequenceEdgeTypes}
            onEdgeClick={onEdgeClick}
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

function SequenceParticipantNode({ data }: ReactFlowNodeProps) {
  const { participant, height, messages, messageGap, messageTop } =
    data as unknown as {
      participant: ActorRef;
      height: number;
      messages: SequenceMessage[];
      messageGap: number;
      messageTop: number;
    };
  return (
    <div
      className="sequence-participant-node"
      style={{ height }}
    >
      <div className="sequence-participant-comment-target">
        <span
          className="sequence-participant-label"
          title={participant.label}
          onClick={(event) => event.stopPropagation()}
        >
          {participant.label}
        </span>
      </div>
      <div className="sequence-lifeline" />
      {messages.flatMap((message, messageIndex) => {
        if (
          message.from.id !== participant.id &&
          message.to.id !== participant.id
        ) {
          return [];
        }
        const top = messageTop + messageIndex * messageGap;
        const isSelfLoop = message.from.id === message.to.id;
        const handles: Array<{
          id: string;
          type: "source" | "target";
          side: Position.Left | Position.Right;
        }> = [];
        if (message.from.id === participant.id) {
          handles.push({
            id: sequenceHandleId(message.id, "source"),
            type: "source",
            side: Position.Right,
          });
        }
        if (message.to.id === participant.id) {
          handles.push({
            id: sequenceHandleId(message.id, "target"),
            type: "target",
            side: isSelfLoop ? Position.Right : Position.Left,
          });
        }
        return handles.map((handle) => (
          <Handle
            key={`${message.id}-${handle.type}`}
            id={handle.id}
            type={handle.type}
            position={handle.side}
            className="sequence-message-handle"
            style={{
              left: "50%",
              top:
                top +
                (isSelfLoop && handle.type === "target" ? 24 : 0),
            }}
          />
        ));
      })}
    </div>
  );
}

function SequenceMessageEdge(props: ReactFlowEdgeProps) {
  const data = props.data as {
    message: SequenceMessage;
    width: number;
    openSource: (anchor: AnchorRef) => void;
  };
  const isSelfLoop = props.source === props.target;
  const loopX =
    props.sourceX + (props.sourceX + 72 > data.width ? -54 : 54);
  const edgePath = isSelfLoop
    ? `M ${props.sourceX} ${props.sourceY} H ${loopX} V ${props.targetY} H ${props.targetX}`
    : getStraightPath({
        sourceX: props.sourceX,
        sourceY: props.sourceY,
        targetX: props.targetX,
        targetY: props.targetY,
      })[0];
  const labelX = isSelfLoop
    ? (props.sourceX + loopX) / 2
    : (props.sourceX + props.targetX) / 2;
  return (
    <>
      <BaseEdge
        id={props.id}
        path={edgePath}
        markerEnd={props.markerEnd}
        className="sequence-message clickable"
        style={props.style}
      />
      <path
        d={edgePath}
        className="sequence-message-hit-area"
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          data.openSource(data.message.anchor);
        }}
      />
      <EdgeLabelRenderer>
        <button
          type="button"
          className="sequence-message-dot"
          style={{
            transform: `translate(-50%, -50%) translate(${props.sourceX}px,${props.sourceY}px)`,
          }}
          onClick={(event) => {
            event.stopPropagation();
            data.openSource(data.message.anchor);
          }}
          aria-label={data.message.label}
        />
        <div
          className="sequence-message-comment-target"
          style={{
            transform: `translate(-50%, -100%) translate(${labelX}px,${props.sourceY - 12}px)`,
          }}
        >
          <span
            role="button"
            tabIndex={0}
            className="sequence-message-label clickable"
            onClick={(event) => {
              event.stopPropagation();
              if (hasTextSelectionWithin(event.currentTarget)) return;
              data.openSource(data.message.anchor);
            }}
            onKeyDown={(event) => {
              if (event.key !== "Enter" && event.key !== " ") return;
              event.preventDefault();
              event.stopPropagation();
              data.openSource(data.message.anchor);
            }}
          >
            {data.message.label}
          </span>
        </div>
      </EdgeLabelRenderer>
    </>
  );
}

interface HorizontalScrollEvent {
  deltaX: number;
  deltaY: number;
  shiftKey: boolean;
}

function scrollDiagramHorizontally(
  scroll: HTMLElement,
  event: HorizontalScrollEvent,
) {
  const delta =
    event.deltaX !== 0 ? event.deltaX : event.shiftKey ? event.deltaY : 0;
  if (delta === 0 || scroll.scrollWidth <= scroll.clientWidth) return false;
  const nextLeft = Math.min(
    Math.max(scroll.scrollLeft + delta, 0),
    scroll.scrollWidth - scroll.clientWidth,
  );
  if (nextLeft === scroll.scrollLeft) return false;
  scroll.scrollLeft = nextLeft;
  return true;
}
function sequenceHandleId(
  messageId: string,
  handleType: "source" | "target",
): string {
  return `${handleType}-${messageId}`;
}

export {
  SequenceDiagramRenderer as Renderer,
  sequenceDiagramPropsSchema as schema,
};
