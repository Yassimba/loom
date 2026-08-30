import { useState } from "react";

const state = new Map<string, unknown>();

export function readReviewUiState<T>(_scope: string, key: string): T | null {
  return (state.get(key) as T | undefined) ?? null;
}

export function writeReviewUiState(_scope: string, key: string, value: unknown): void {
  state.set(key, value);
}

export function forgetReviewUiState(_scope: string, predicate: string | ((key: string) => boolean)): void {
  if (typeof predicate === "string") state.delete(predicate);
  else for (const key of state.keys()) if (predicate(key)) state.delete(key);
}

export function useReviewUiState<T>(_key: string, initialValue: T) {
  return useState(initialValue);
}
