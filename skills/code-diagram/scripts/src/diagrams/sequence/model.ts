// Ported from devdotfast/review@8267620:
// packages/progressive-review/app/src/diagrams.tsx (pure sequence model).
import {
  type ActorRef,
  type AnchorRef,
  throwAuthoringIssue,
} from "../../authoring/core";
import {
  type SequenceActorInput,
  type SequenceDiagramProps,
} from "./authoring";

export interface SequenceMessage {
  id: string;
  from: ActorRef;
  to: ActorRef;
  label: string;
  anchor: AnchorRef;
}

export function createSequence(input: SequenceDiagramProps) {
  const id = `sequence-${slugSequenceActorLabel(input.label)}`;
  const messages = normalizeSequenceMessages(input, id);
  const participants = participantsForMessages(messages);
  validateSequenceTargetPaths(input.label, participants, messages);
  return Object.freeze({
    participants,
    messages,
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
  input: SequenceDiagramProps,
  sequenceId: string,
): SequenceMessage[] {
  const { messages } = input;
  const explicitActorIds = new Set<string>();
  for (const message of messages) {
    if (message.from.id) explicitActorIds.add(message.from.id);
    if (message.to.id) explicitActorIds.add(message.to.id);
  }

  const inlineActorsByLabel = new Map<string, ActorRef>();
  const usedActorIds = new Set(explicitActorIds);

  const normalizeActor = (actor: SequenceActorInput): ActorRef => {
    if ("__kind" in actor) return actor;
    if (actor.id) {
      return {
        __kind: "db-actor-ref",
        id: actor.id,
        label: actor.label,
      };
    }

    const existing = inlineActorsByLabel.get(actor.label);
    if (existing) return existing;

    const baseId = `inline-${slugSequenceActorLabel(actor.label) || "actor"}`;
    let id = baseId;
    for (let suffix = 2; usedActorIds.has(id); suffix += 1) {
      id = `${baseId}-${suffix}`;
    }
    usedActorIds.add(id);
    const normalized: ActorRef = {
      __kind: "db-actor-ref",
      id,
      label: actor.label,
    };
    inlineActorsByLabel.set(actor.label, normalized);
    return normalized;
  };

  return messages.map((message, index) => {
    const anchor =
      message.anchor ??
      ({
        __kind: "db-anchor-ref",
        id: `sequence-${slugSequenceActorLabel(input.label) || "diagram"}-message-${index + 1}`,
        title: message.label,
      } satisfies AnchorRef);
    return Object.freeze<SequenceMessage>({
      id: `${sequenceId}-message-${index + 1}-${anchor.id}`,
      from: normalizeActor(message.from),
      to: normalizeActor(message.to),
      label: message.label,
      anchor,
    });
  });
}

function slugSequenceActorLabel(label: string) {
  return label
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function participantsForMessages(
  messages: readonly SequenceMessage[],
): ActorRef[] {
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

  for (const [id, participant] of participants) {
    if (!consumed.has(id)) ordered.push(participant.actor);
  }
  return ordered;
}
