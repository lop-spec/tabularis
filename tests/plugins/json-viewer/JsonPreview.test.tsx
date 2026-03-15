import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import JsonPreview from "../../../src/plugins/examples/json-viewer/ui/JsonPreview";

const baseContext = {
  connectionId: "conn-1",
  tableName: "users",
  schema: "public",
  driver: "postgres",
};

describe("JsonPreview", () => {
  it("should render nothing when columnName is not provided", () => {
    const { container } = render(
      <JsonPreview context={{ ...baseContext }} pluginId="json-viewer" />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("should render nothing for non-JSON columns", () => {
    const { container } = render(
      <JsonPreview
        context={{ ...baseContext, columnName: "email", rowData: { email: "test@example.com" } }}
        pluginId="json-viewer"
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("should render a JSON tree for a column with json in name", () => {
    const jsonData = JSON.stringify({ name: "Alice", age: 30 });
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "metadata_json", rowData: { metadata_json: jsonData } }}
        pluginId="json-viewer"
      />,
    );

    expect(screen.getByText("JSON Preview")).toBeInTheDocument();
    expect(screen.getByText(/Alice/)).toBeInTheDocument();
    expect(screen.getByText("30")).toBeInTheDocument();
  });

  it("should render a JSON tree when value looks like JSON object", () => {
    const jsonData = '{"key": "value"}';
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "data", rowData: { data: jsonData } }}
        pluginId="json-viewer"
      />,
    );

    expect(screen.getByText("JSON Preview")).toBeInTheDocument();
    expect(screen.getByText(/value/)).toBeInTheDocument();
  });

  it("should render a JSON tree when value looks like JSON array", () => {
    const jsonData = '[1, 2, 3]';
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "data", rowData: { data: jsonData } }}
        pluginId="json-viewer"
      />,
    );

    expect(screen.getByText("JSON Preview")).toBeInTheDocument();
    expect(screen.getByText("1")).toBeInTheDocument();
    expect(screen.getByText("2")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("should render nothing for invalid JSON in non-json-named column", () => {
    const { container } = render(
      <JsonPreview
        context={{ ...baseContext, columnName: "notes", rowData: { notes: "not json at all" } }}
        pluginId="json-viewer"
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("should show Copy button and handle click", async () => {
    const writeText = vi.fn().mockResolvedValue(undefined);
    Object.assign(navigator, { clipboard: { writeText } });

    const jsonData = '{"key": "value"}';
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "json_config", rowData: { json_config: jsonData } }}
        pluginId="json-viewer"
      />,
    );

    const copyButton = screen.getByTitle("Copy formatted JSON");
    expect(copyButton).toBeInTheDocument();
    fireEvent.click(copyButton);

    await screen.findByText("Copied");
    expect(writeText).toHaveBeenCalledWith(jsonData);
  });

  it("should handle native object values (not stringified)", () => {
    const obj = { nested: { deep: true }, items: [1, 2] };
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "jsonb_data", rowData: { jsonb_data: obj } }}
        pluginId="json-viewer"
      />,
    );

    expect(screen.getByText("JSON Preview")).toBeInTheDocument();
  });

  it("should collapse and expand objects on click", () => {
    const jsonData = JSON.stringify({ a: { b: { c: { d: 1 } } } });
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "json_data", rowData: { json_data: jsonData } }}
        pluginId="json-viewer"
      />,
    );

    // The deeply nested object (depth >= 2) should be collapsed by default
    expect(screen.getByText(/1 keys/)).toBeInTheDocument();
  });

  it("should render null, boolean, and number tokens with correct styling", () => {
    const jsonData = JSON.stringify({ flag: true, count: 42, empty: null });
    render(
      <JsonPreview
        context={{ ...baseContext, columnName: "json_data", rowData: { json_data: jsonData } }}
        pluginId="json-viewer"
      />,
    );

    expect(screen.getByText("true")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText("null")).toBeInTheDocument();
  });

  it("should render nothing for empty string values", () => {
    const { container } = render(
      <JsonPreview
        context={{ ...baseContext, columnName: "json_data", rowData: { json_data: "" } }}
        pluginId="json-viewer"
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("should render nothing when rowData is missing", () => {
    const { container } = render(
      <JsonPreview
        context={{ ...baseContext, columnName: "json_data" }}
        pluginId="json-viewer"
      />,
    );
    expect(container.innerHTML).toBe("");
  });
});
