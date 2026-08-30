export interface CodeEvidence {
  file: string;
  fromLine: number;
  toLine: number;
}

export interface SequenceMessageInput {
  id?: string;
  from: string;
  to: string;
  label: string;
  evidence?: CodeEvidence;
  code?: string | { language?: string; text: string };
}

export interface SequenceDiagramInput {
  type: "sequence";
  id?: string;
  label: string;
  actors: Record<string, { label: string }>;
  messages: SequenceMessageInput[];
}

export interface CodeDiagramDocumentInput {
  version: 1;
  title: string;
  intro?: string[];
  diagrams: SequenceDiagramInput[];
}

export declare class CodeDiagramInputError extends Error {
  readonly code: string;
  readonly path: string;
}

export declare function defineDocument(input: CodeDiagramDocumentInput): Readonly<CodeDiagramDocumentInput>;
export declare function normalizeDocument(input: unknown): Readonly<CodeDiagramDocumentInput>;
