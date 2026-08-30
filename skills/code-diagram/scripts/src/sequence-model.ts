// Ported from devdotfast/review@8267620:
// packages/progressive-review/app/src/diagrams.tsx (pure sequence model).
import {
  type ActorRef,
  type AnchorRef,
  type SequenceActorInput,
  type SequenceDiagramProps,
  type SequenceMessageCodeInput,
  sequenceDiagramPropsSchema,
  throwAuthoringIssue,
} from "./authoring";

interface UncheckedSequenceMessageInput {
  from: SequenceActorInput;
  to: SequenceActorInput;
  label: string;
  anchor?: AnchorRef;
  code?: SequenceMessageCodeInput;
}

interface UncheckedSequenceInput {
  label: string;
  messages: UncheckedSequenceMessageInput[];
}

export interface SequenceMessageCodeBlock {
  language?: string;
  text: string;
}

export interface SequenceMessage {
  id: string;
  from: ActorRef;
  to: ActorRef;
  label: string;
  anchor: AnchorRef;
  code?: SequenceMessageCodeBlock;
}

export interface SequenceRef {
  __kind: "review-sequence-ref";
  id: string;
  label: string;
  participants: ActorRef[];
  messages: SequenceMessage[];
}

export function createSequence(input: UncheckedSequenceInput): SequenceRef {
  sequenceDiagramPropsSchema.parse(input);
  const messages = uniqueSequenceMessageAnchors(
    normalizeSequenceMessages(input),
  );
  const id = `sequence-${slugSequenceActorLabel(input.label)}`;
  const participants = participantsForMessages(messages);
  validateSequenceTargetPaths(input.label, participants, messages);
  return Object.freeze({
    __kind: "review-sequence-ref",
    id,
    label: input.label,
    participants,
    messages: messages.map((message, index) =>
      Object.freeze({
        id: `${id}-message-${index + 1}-${message.anchor.id}`,
        from: message.from,
        to: message.to,
        label: message.label,
        anchor: message.anchor,
        code: message.code,
      }),
    ),
  });
}

function uniqueSequenceMessageAnchors(
  messages: readonly SequenceMessage[],
): SequenceMessage[] {
  const reservedIds = new Set(messages.map((message) => message.anchor.id));
  const usedIds = new Set<string>();
  return messages.map((message, index) => {
    if (!usedIds.has(message.anchor.id)) {
      usedIds.add(message.anchor.id);
      return message;
    }
    const prefix = `${message.anchor.id}--sequence-use-${index + 1}`;
    let id = prefix;
    let suffix = 2;
    while (reservedIds.has(id) || usedIds.has(id)) {
      id = `${prefix}-${suffix}`;
      suffix += 1;
    }
    usedIds.add(id);
    return {
      ...message,
      anchor: Object.freeze({ ...message.anchor, id }),
    };
  });
}

function validateSequenceTargetPaths(
  diagram: string,
  participants: readonly ActorRef[],
  messages: readonly SequenceMessage[],
): void {
  const participantLabels = new Set<string>();
  for (const participant of participants) {
    if (participantLabels.has(participant.label)) {
      throwAuthoringIssue(
        ["messages"],
        `SequenceDiagram "${diagram}" has more than one participant labelled "${participant.label}"`,
      );
    }
    participantLabels.add(participant.label);
  }
  const parallelLabels = new Map<string, Set<string>>();
  for (const [index, message] of messages.entries()) {
    const segment = `${message.from.label}→${message.to.label}`;
    const labels = parallelLabels.get(segment) ?? new Set<string>();
    if (labels.has(message.label)) {
      throwAuthoringIssue(
        ["messages", index, "label"],
        `Label must be unique among parallel ${segment} messages`,
      );
    }
    labels.add(message.label);
    parallelLabels.set(segment, labels);
  }
}

function normalizeSequenceMessages(
  input: UncheckedSequenceInput,
): SequenceMessage[] {
  const { messages } = input;
  const explicitActorIds = new Set<string>();
  for (const message of messages) {
    if (message.from.id) explicitActorIds.add(message.from.id);
    if (message.to.id) explicitActorIds.add(message.to.id);
  }

  const inlineActorIdsByLabel = new Map<string, string>();
  const usedActorIds = new Set(explicitActorIds);

  const normalizeActor = (actor: SequenceActorInput): ActorRef => {
    if (actor.id) {
      return {
        __kind: "db-actor-ref",
        id: actor.id,
        label: actor.label,
        softwareMapPath: actorSoftwareMapPath(actor),
      };
    }

    const existing = inlineActorIdsByLabel.get(actor.label);
    if (existing) {
      return {
        __kind: "db-actor-ref",
        id: existing,
        label: actor.label,
        softwareMapPath: actorSoftwareMapPath(actor),
      };
    }

    const baseId = `inline-${slugSequenceActorLabel(actor.label) || "actor"}`;
    let id = baseId;
    for (let suffix = 2; usedActorIds.has(id); suffix += 1) {
      id = `${baseId}-${suffix}`;
    }
    usedActorIds.add(id);
    inlineActorIdsByLabel.set(actor.label, id);
    return {
      __kind: "db-actor-ref",
      id,
      label: actor.label,
      softwareMapPath: actorSoftwareMapPath(actor),
    };
  };

  return messages.map((message, index) => {
    const code = normalizeSequenceMessageCode(message.code);
    const fallbackAnchor = {
      __kind: "db-anchor-ref",
      id: `sequence-${slugSequenceActorLabel(input.label) || "diagram"}-message-${index + 1}`,
      title: message.label,
    } satisfies AnchorRef;
    return Object.freeze({
      id: `sequence-message-input-${index + 1}`,
      from: normalizeActor(message.from),
      to: normalizeActor(message.to),
      label: message.label,
      anchor: message.anchor ?? fallbackAnchor,
      code,
    });
  });
}

function actorSoftwareMapPath(actor: SequenceActorInput): string | undefined {
  return "__kind" in actor ? actor.softwareMapPath : undefined;
}

function normalizeSequenceMessageCode(
  code: SequenceMessageCodeInput | undefined,
): SequenceMessageCodeBlock | undefined {
  if (typeof code === "string") {
    const text = code.trim();
    return text ? { text } : undefined;
  }
  if (!code) return undefined;
  const text = code.text.trim();
  if (!text) return undefined;
  const language = code.language?.trim();
  return language ? { language, text } : { text };
}

function slugSequenceActorLabel(label: string) {
  return label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function participantsForMessages(messages: SequenceMessage[]): ActorRef[] {
  const participants = new Map<string, { actor: ActorRef; order: number }>();
  const outgoing = new Map<string, Set<string>>();
  const incomingCount = new Map<string, number>();
  for (const message of messages) {
    if (!participants.has(message.from.id)) {
      participants.set(message.from.id, {
        actor: message.from,
        order: participants.size,
      });
      incomingCount.set(message.from.id, 0);
    }
    if (!participants.has(message.to.id)) {
      participants.set(message.to.id, {
        actor: message.to,
        order: participants.size,
      });
      incomingCount.set(message.to.id, 0);
    }
    if (message.from.id === message.to.id) continue;
    const targets = outgoing.get(message.from.id) ?? new Set<string>();
    if (!targets.has(message.to.id)) {
      targets.add(message.to.id);
      outgoing.set(message.from.id, targets);
      incomingCount.set(
        message.to.id,
        (incomingCount.get(message.to.id) ?? 0) + 1,
      );
    }
  }
  const byFirstSeen = (left: string, right: string) =>
    (participants.get(left)?.order ?? 0) -
    (participants.get(right)?.order ?? 0);
  const ready = [...participants.keys()]
    .filter((id) => (incomingCount.get(id) ?? 0) === 0)
    .sort(byFirstSeen);
  const ordered: ActorRef[] = [];
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

export type SequenceInput = SequenceDiagramProps;
