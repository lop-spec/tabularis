export interface DataTypeInfo {
  name: string;
  category: "numeric" | "string" | "date" | "binary" | "json" | "spatial" | "other";
  requires_length: boolean;
  requires_precision: boolean;
  default_length?: string;
}

export interface DataTypeRegistry {
  driver: string;
  types: DataTypeInfo[];
}
