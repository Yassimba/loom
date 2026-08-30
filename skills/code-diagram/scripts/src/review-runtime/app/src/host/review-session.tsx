export interface ReviewSession {
  storageKey(...keys: string[]): string;
  theme(): "dark" | "light";
  apiUrl(path: string): string;
  fetchUrl(path: string, init?: RequestInit): Promise<Response>;
  wasmUrl(path?: string): string;
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
  surface: { subscribe(listener: (event: { event: string; theme: "dark" | "light" }) => void): () => void };
  [key: string]: unknown;
}

const session: ReviewSession = {
  storageKey: (...keys) => `offline:${keys.join(":")}`,
  theme: () => "dark",
  apiUrl: (path) => path,
  fetchUrl: (path, init) => globalThis.fetch(path, init),
  wasmUrl: (path = "") =>
    (globalThis as { __CODE_DIAGRAM_LIBAVOID_WASM_URL__?: string })
      .__CODE_DIAGRAM_LIBAVOID_WASM_URL__ ?? path,
  fetch: (input, init) => globalThis.fetch(input, init),
  surface: { subscribe: () => () => undefined },
};

export function useReviewSession(): ReviewSession {
  return session;
}
