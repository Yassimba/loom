import { useEffect, useRef, type ReactNode } from "react";
import type { AnchorRef } from "../../src/authoring";
import type { SourceRange } from "../../../diagram-family";
import type { ValidatedCodePeekInput } from "./CodePeek";

export type ReviewPeekContent =
  | { kind: "resolved-code"; input: ValidatedCodePeekInput }
  | { kind: "inline-code"; language?: string; text: string }
  | { kind: "trace-quote"; sessionId: string; trace?: string; event?: number; quote: string };

export interface GuidedTourStop {
  anchor: AnchorRef;
  label: string;
  detail?: string;
  content: ReviewPeekContent;
}

export interface GuidedTour {
  id: string;
  title?: string;
  stops: GuidedTourStop[];
  telemetryKind?: "sequence";
}

export function GuidedTourPanel({
  tour,
  activeAnchor,
  revealRequest,
  onActiveAnchorChange,
  onClose,
}: {
  tour: GuidedTour;
  activeAnchor: string;
  revealRequest: number;
  onActiveAnchorChange: (anchor: string, options: { reveal: boolean }) => void;
  onClose: () => void;
}) {
  const scrollerRef = useRef<HTMLDivElement | null>(null);
  const sectionRefs = useRef(new Map<string, HTMLElement>());
  const activeIndex = Math.max(0, tour.stops.findIndex((stop) => stop.anchor.id === activeAnchor));
  useEffect(() => {
    if (!revealRequest) return;
    const scroller = scrollerRef.current;
    const section = sectionRefs.current.get(activeAnchor);
    if (!scroller || !section) return;
    scroller.scrollTo({
      top:
        scroller.scrollTop +
        section.getBoundingClientRect().top -
        scroller.getBoundingClientRect().top,
      behavior: "smooth",
    });
  }, [activeAnchor, revealRequest]);
  const syncActiveStopToScroll = () => {
    const scroller = scrollerRef.current;
    const lastStop = tour.stops.at(-1);
    if (!scroller || !lastStop) return;
    const bounds = scroller.getBoundingClientRect();
    const activeLine = bounds.top + bounds.height / 2;
    let nextAnchor = lastStop.anchor.id;
    for (const [index, stop] of tour.stops.entries()) {
      const section = sectionRefs.current.get(stop.anchor.id);
      if (!section) continue;
      const top = section.getBoundingClientRect().top;
      if (top < bounds.top) continue;
      nextAnchor = top <= activeLine ? stop.anchor.id : (tour.stops[index - 1] ?? stop).anchor.id;
      break;
    }
    if (nextAnchor !== activeAnchor) onActiveAnchorChange(nextAnchor, { reveal: false });
  };
  const stepTo = (index: number) => {
    const stop = tour.stops[index];
    if (stop) onActiveAnchorChange(stop.anchor.id, { reveal: true });
  };
  return (
    <section className="side-peek side-peek--tour" aria-label={tour.title ?? "Guided tour"}>
      <header className="side-peek-header">
        <div>
          <span className="side-peek-label">Tour</span>
          <h2>{tour.title ?? "Guided tour"}</h2>
        </div>
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close guided tour">
          ×
        </button>
      </header>
      <div className="tour-feed-shell">
        <div ref={scrollerRef} className="side-peek-body tour-feed" onScroll={syncActiveStopToScroll}>
          {tour.stops.map((stop, index) => {
          const active = stop.anchor.id === activeAnchor;
          return (
            <section
              key={stop.anchor.id}
              ref={(node) => {
                if (node) sectionRefs.current.set(stop.anchor.id, node);
                else sectionRefs.current.delete(stop.anchor.id);
              }}
              className={active ? "tour-stop active" : "tour-stop"}
              data-review-anchor-id={stop.anchor.id}
              onClick={() => onActiveAnchorChange(stop.anchor.id, { reveal: false })}
            >
              <div className="tour-stop-rail"><div>{index + 1}</div></div>
              <div className="tour-stop-main">
                <header className="tour-stop-header">
                  <div>
                    <div className="tour-stop-count">Step {index + 1} of {tour.stops.length}</div>
                    <div className="tour-stop-title-row"><h3>{stop.label}</h3></div>
                    {stop.detail ? <p>{stop.detail}</p> : null}
                  </div>
                </header>
                <div className="peek-content"><TourContent content={stop.content} /></div>
              </div>
            </section>
          );
        })}
          <div className="tour-end-cap"><span>End of tour</span><button type="button" onClick={() => stepTo(0)}>↑ Back to step 1</button></div>
          <div className="tour-scroll-tail" aria-hidden="true" />
        </div>
      </div>
      <div className="tour-floating-footer">
        <div className="tour-pill" role="group" aria-label="Tour steps">
          <button type="button" aria-label="Previous step" disabled={activeIndex === 0} onClick={() => stepTo(activeIndex - 1)}>↑</button>
          <span className="tour-pill-count" aria-live="polite">{activeIndex + 1}/{tour.stops.length}</span>
          <button type="button" aria-label="Next step" disabled={activeIndex >= tour.stops.length - 1} onClick={() => stepTo(activeIndex + 1)}>↓</button>
        </div>
      </div>
    </section>
  );
}

function TourContent({ content }: { content: ReviewPeekContent }) {
  if (content.kind === "inline-code") return <pre><code>{content.text}</code></pre>;
  if (content.kind === "trace-quote") return <blockquote>{content.quote}</blockquote>;
  const source = sourceFromInput(content.input);
  if (!source) return <p>Source evidence unavailable.</p>;
  return (
    <div className="code-peek-card">
      <div className="code-peek-file">{source.file}:{source.fromLine}-{source.toLine}</div>
      <pre><code>{source.lines.map((line) => <span className="code-diagram-source-line" key={line.number}><span className="code-diagram-line-number">{line.number}</span><span>{line.text || " "}</span></span>)}</code></pre>
    </div>
  );
}

function sourceFromInput(input: unknown): SourceRange | null {
  if (!input || typeof input !== "object") return null;
  const candidate = "source" in input ? (input as { source?: unknown }).source : input;
  if (!candidate || typeof candidate !== "object" || !("lines" in candidate)) return null;
  return candidate as SourceRange;
}

export function ReviewPanelHost(): ReactNode {
  return null;
}
