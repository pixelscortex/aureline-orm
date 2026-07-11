export type Scalar = "string" | "integer" | "boolean";

export interface Field {
  readonly name: string;
  readonly type: Scalar | { readonly model: string };
  readonly required: boolean;
}

export interface Model {
  readonly name: string;
  readonly fields: readonly Field[];
}

export interface Schema {
  readonly name: string;
  readonly models: readonly Model[];
}

export function createSchema(
  name: string,
  models: readonly Model[] = [],
): Schema {
  return { name, models };
}
