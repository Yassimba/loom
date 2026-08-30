// Review parity preview authoring contract.
// Source model: devdotfast/review@8267620, packages/progressive-review/src/authoring.ts.

const DOCUMENT_KEYS = new Set(["version", "title", "intro", "diagrams"]);
const DIAGRAM_KEYS = new Set(["type", "id", "label", "actors", "messages"]);
const ACTOR_KEYS = new Set(["label"]);
const MESSAGE_KEYS = new Set(["id", "from", "to", "label", "evidence", "code"]);
const EVIDENCE_KEYS = new Set(["file", "fromLine", "toLine"]);
const CODE_KEYS = new Set(["language", "text"]);

export class CodeDiagramInputError extends Error {
  constructor(code, path, message) {
    super(`${path}: ${message}`);
    this.name = "CodeDiagramInputError";
    this.code = code;
    this.path = path;
  }
}

export function defineDocument(input) {
  normalizeDocument(input);
  return freeze(input);
}

export function normalizeDocument(input) {
  objectAt(input, "document");
  exactKeys(input, DOCUMENT_KEYS, "document");
  if (input.version !== 1) fail("SCHEMA_VERSION", "document.version", "must be 1");
  const title = stringAt(input.title, "document.title");
  const intro = input.intro === undefined ? [] : stringArrayAt(input.intro, "document.intro");
  if (!Array.isArray(input.diagrams) || input.diagrams.length === 0) {
    fail("DIAGRAMS_EMPTY", "document.diagrams", "must contain at least one diagram");
  }

  const ids = new Set();
  const diagrams = input.diagrams.map((diagram, index) => {
    const normalized = normalizeSequence(diagram, index);
    if (ids.has(normalized.id)) {
      fail("IDENTITY_DUPLICATE", `document.diagrams[${index}].id`, `duplicate diagram id ${JSON.stringify(normalized.id)}`);
    }
    ids.add(normalized.id);
    return normalized;
  });

  return freeze({ version: 1, title, intro, diagrams });
}

function normalizeSequence(input, index) {
  const path = `document.diagrams[${index}]`;
  objectAt(input, path);
  exactKeys(input, DIAGRAM_KEYS, path);
  if (input.type !== "sequence") {
    fail("DIAGRAM_UNSUPPORTED", `${path}.type`, "Sequence Diagram is the only supported surface in this preview");
  }
  const label = stringAt(input.label, `${path}.label`);
  const id = input.id === undefined ? `sequence-${slug(label) || index + 1}` : idAt(input.id, `${path}.id`);
  objectAt(input.actors, `${path}.actors`);
  const actorEntries = Object.entries(input.actors);
  if (actorEntries.length === 0) fail("ACTORS_EMPTY", `${path}.actors`, "must contain at least one actor");
  const labels = new Set();
  const actors = Object.fromEntries(actorEntries.map(([actorId, actor]) => {
    idAt(actorId, `${path}.actors key`);
    objectAt(actor, `${path}.actors.${actorId}`);
    exactKeys(actor, ACTOR_KEYS, `${path}.actors.${actorId}`);
    const actorLabel = stringAt(actor.label, `${path}.actors.${actorId}.label`);
    if (labels.has(actorLabel)) {
      fail("ACTOR_LABEL_DUPLICATE", `${path}.actors.${actorId}.label`, `actor label ${JSON.stringify(actorLabel)} is already used`);
    }
    labels.add(actorLabel);
    return [actorId, freeze({ id: actorId, label: actorLabel })];
  }));
  if (!Array.isArray(input.messages) || input.messages.length === 0) {
    fail("MESSAGES_EMPTY", `${path}.messages`, "must contain at least one message");
  }
  const messageIds = new Set();
  const parallelLabels = new Map();
  const messages = input.messages.map((message, messageIndex) => {
    const messagePath = `${path}.messages[${messageIndex}]`;
    objectAt(message, messagePath);
    exactKeys(message, MESSAGE_KEYS, messagePath);
    const from = idAt(message.from, `${messagePath}.from`);
    const to = idAt(message.to, `${messagePath}.to`);
    if (!actors[from]) fail("ENDPOINT_UNKNOWN", `${messagePath}.from`, `unknown actor ${JSON.stringify(from)}`);
    if (!actors[to]) fail("ENDPOINT_UNKNOWN", `${messagePath}.to`, `unknown actor ${JSON.stringify(to)}`);
    const messageLabel = stringAt(message.label, `${messagePath}.label`);
    const messageId = message.id === undefined ? `${id}-message-${messageIndex + 1}` : idAt(message.id, `${messagePath}.id`);
    if (messageIds.has(messageId)) fail("IDENTITY_DUPLICATE", `${messagePath}.id`, `duplicate message id ${JSON.stringify(messageId)}`);
    messageIds.add(messageId);
    const segment = `${actors[from].label}→${actors[to].label}`;
    const segmentLabels = parallelLabels.get(segment) ?? new Set();
    if (segmentLabels.has(messageLabel)) {
      fail("MESSAGE_LABEL_DUPLICATE", `${messagePath}.label`, `label must be unique among parallel ${segment} messages`);
    }
    segmentLabels.add(messageLabel);
    parallelLabels.set(segment, segmentLabels);
    const evidence = message.evidence === undefined ? undefined : normalizeEvidence(message.evidence, `${messagePath}.evidence`);
    const code = message.code === undefined ? undefined : normalizeCode(message.code, `${messagePath}.code`);
    if (!evidence && !code) {
      fail("EVIDENCE_MISSING", messagePath, "message needs evidence or inline code");
    }
    return freeze({ id: messageId, from, to, label: messageLabel, evidence, code });
  });
  return freeze({ type: "sequence", id, label, actors: freeze(actors), messages });
}

function normalizeEvidence(input, path) {
  objectAt(input, path);
  exactKeys(input, EVIDENCE_KEYS, path);
  const file = stringAt(input.file, `${path}.file`);
  if (file.startsWith("/") || file.split(/[\\/]/).includes("..")) {
    fail("EVIDENCE_FILE_INVALID", `${path}.file`, "must be a repository-relative path without .. segments");
  }
  const fromLine = positiveIntegerAt(input.fromLine, `${path}.fromLine`);
  const toLine = positiveIntegerAt(input.toLine, `${path}.toLine`);
  if (toLine < fromLine) fail("EVIDENCE_RANGE_INVALID", `${path}.toLine`, "must be greater than or equal to fromLine");
  return freeze({ file, fromLine, toLine });
}

function normalizeCode(input, path) {
  if (typeof input === "string") return freeze({ text: stringAt(input, path) });
  objectAt(input, path);
  exactKeys(input, CODE_KEYS, path);
  const text = stringAt(input.text, `${path}.text`);
  const language = input.language === undefined ? undefined : stringAt(input.language, `${path}.language`);
  return freeze(language ? { language, text } : { text });
}

function exactKeys(input, allowed, path) {
  for (const key of Object.keys(input)) {
    if (!allowed.has(key)) fail("SCHEMA_UNKNOWN_FIELD", `${path}.${key}`, "unknown field");
  }
}

function objectAt(input, path) {
  if (!input || typeof input !== "object" || Array.isArray(input)) fail("SCHEMA_TYPE", path, "must be an object");
  return input;
}

function stringAt(input, path) {
  if (typeof input !== "string" || input.trim().length === 0) fail("SCHEMA_TYPE", path, "must be a non-empty string");
  return input.trim();
}

function stringArrayAt(input, path) {
  if (!Array.isArray(input)) fail("SCHEMA_TYPE", path, "must be an array of strings");
  return input.map((value, index) => stringAt(value, `${path}[${index}]`));
}

function positiveIntegerAt(input, path) {
  if (!Number.isInteger(input) || input <= 0) fail("SCHEMA_TYPE", path, "must be a positive integer");
  return input;
}

function idAt(input, path) {
  const value = stringAt(input, path);
  if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(value)) fail("IDENTITY_INVALID", path, "must contain only letters, numbers, dot, underscore, or dash");
  return value;
}

function slug(value) {
  return value.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "");
}

function freeze(value) {
  if (value && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}

function fail(code, path, message) {
  throw new CodeDiagramInputError(code, path, message);
}
