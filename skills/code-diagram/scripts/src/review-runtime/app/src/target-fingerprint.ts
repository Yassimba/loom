export function buildGraphTarget(input: Record<string, unknown>): any {
  return { kind: "graph" as const, ...input };
}

export function targetKey(target: unknown): string {
  return JSON.stringify(target);
}
