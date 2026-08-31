export type DataStoreForeignKeyRef =
  | string
  | {
      table: string;
      field: string;
      label?: string;
      cardinality?: "one-to-one" | "many-to-one";
      onDelete?: string;
      onUpdate?: string;
    };

export interface DataStoreFieldLeaf {
  type: string;
  example?: unknown;
  pk?: boolean;
  fk?: DataStoreForeignKeyRef;
  schema?: DataStoreFieldSchema;
}

export type DataStoreFieldSchema = {
  [field: string]: DataStoreFieldLeaf | DataStoreFieldSchema;
};

export interface DataStoreSchemaRow {
  path: string[];
  label: string;
  depth: number;
  type?: string;
  pk?: boolean;
  fk?: DataStoreForeignKeyRef;
  example?: unknown;
}

export function isDataStoreFieldLeaf(value: unknown): value is DataStoreFieldLeaf {
  return (
    value !== null &&
    typeof value === "object" &&
    typeof (value as { type?: unknown }).type === "string"
  );
}

export function parseDataStoreForeignKeyRef(
  reference: DataStoreForeignKeyRef,
): { table: string; fieldPath: string[] } | undefined {
  if (typeof reference === "string") {
    const [table, ...fieldPath] = reference.split(".").filter(Boolean);
    return table && fieldPath.length ? { table, fieldPath } : undefined;
  }
  const fieldPath = reference.field.split(".").filter(Boolean);
  return reference.table && fieldPath.length
    ? { table: reference.table, fieldPath }
    : undefined;
}

export function flattenDataStoreSchema(schema: DataStoreFieldSchema): DataStoreSchemaRow[] {
  const rows: DataStoreSchemaRow[] = [];
  const visit = (node: DataStoreFieldSchema, prefix: string[], depth: number) => {
    for (const [label, value] of Object.entries(node)) {
      const path = [...prefix, label];
      const leaf = isDataStoreFieldLeaf(value);
      rows.push({
        path,
        label,
        depth,
        type: leaf ? value.type : undefined,
        pk: leaf ? value.pk : undefined,
        fk: leaf ? value.fk : undefined,
        example: dataStoreSchemaExample(value),
      });
      const nested = leaf ? value.schema : value;
      if (nested) visit(nested, path, depth + 1);
    }
  };
  visit(schema, [], 0);
  return rows;
}

export function dataStoreSchemaExample(
  value: DataStoreFieldLeaf | DataStoreFieldSchema,
): unknown {
  if (isDataStoreFieldLeaf(value)) {
    if ("example" in value) return value.example;
    return value.schema ? dataStoreSchemaExample(value.schema) : undefined;
  }
  return Object.fromEntries(
    Object.entries(value).map(([key, child]) => [key, dataStoreSchemaExample(child)]),
  );
}

export function formatDataStoreExample(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value === "string") return value;
  return typeof value === "object" ? JSON.stringify(value) : String(value);
}
