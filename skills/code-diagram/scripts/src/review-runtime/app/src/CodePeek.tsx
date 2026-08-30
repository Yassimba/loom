import type { CodePeekRef } from "../../src/authoring";

export type ValidatedCodePeekInput = unknown;

export function validatedCodePeekInputFromRef(ref: CodePeekRef): ValidatedCodePeekInput {
  return ref.resolution ?? ref.props;
}

export function CodePeekGroup(_props: { peeks: readonly unknown[]; collapsed: boolean }) {
  return null;
}
