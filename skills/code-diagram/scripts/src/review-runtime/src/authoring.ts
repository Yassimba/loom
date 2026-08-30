export * from "../../authoring";
export type DbOperationProps =
  | import("../../authoring").DbReadProps
  | import("../../authoring").DbWriteProps;
