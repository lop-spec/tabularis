import { describe, expect, it } from "vitest";
import {
  buildShowCreateTableQuery,
  extractCreateTableSql,
  supportsShowCreateTable,
} from "../../src/utils/showCreateTable";

describe("showCreateTable", () => {
  it("uses native SHOW CREATE TABLE for MySQL-compatible drivers", () => {
    expect(supportsShowCreateTable("mysql")).toBe(true);
    expect(supportsShowCreateTable("mariadb")).toBe(true);
    expect(supportsShowCreateTable("postgres")).toBe(false);
    expect(supportsShowCreateTable(null)).toBe(false);
  });

  it("quotes the schema and table as MySQL identifiers", () => {
    expect(buildShowCreateTableQuery("order`items", "sales`archive")).toBe(
      "SHOW CREATE TABLE `sales``archive`.`order``items`;",
    );
  });

  it("extracts the server-native DDL and makes it executable", () => {
    expect(
      extractCreateTableSql({
        columns: ["Table", "Create Table"],
        rows: [["orders", "CREATE TABLE `orders` (`id` bigint) ENGINE=InnoDB"]],
      }),
    ).toBe("CREATE TABLE `orders` (`id` bigint) ENGINE=InnoDB;");
  });

  it("rejects a SHOW result without a CREATE TABLE value", () => {
    expect(() =>
      extractCreateTableSql({ columns: ["Table"], rows: [["orders"]] }),
    ).toThrow("SHOW CREATE TABLE did not return a CREATE TABLE statement");
  });
});
