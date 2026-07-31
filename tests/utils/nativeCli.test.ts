import { describe, expect, it } from "vitest";
import {
  findNativeCliCommandAtOffset,
  splitNativeCliCommands,
} from "../../src/utils/nativeCli";

describe("splitNativeCliCommands", () => {
  it("splits redis-cli input by non-empty lines without rewriting arguments", () => {
    const input = 'SET greeting "hello world"\n\nGET greeting\nMONITOR';
    expect(splitNativeCliCommands(input, "redis-cli").map((item) => item.text)).toEqual([
      'SET greeting "hello world"',
      "GET greeting",
      "MONITOR",
    ]);
  });

  it("keeps multiline mongosh JavaScript together and splits complete commands", () => {
    const input = [
      "db.orders.aggregate([",
      "  { $match: { note: 'a;b' } },",
      "  { $limit: 10 }",
      "]);",
      "db.orders.countDocuments({});",
    ].join("\n");

    expect(splitNativeCliCommands(input, "mongosh").map((item) => item.text)).toEqual([
      [
        "db.orders.aggregate([",
        "  { $match: { note: 'a;b' } },",
        "  { $limit: 10 }",
        "]);",
      ].join("\n"),
      "db.orders.countDocuments({});",
    ]);
  });

  it("does not split mongosh semicolons inside regex, templates, or comments", () => {
    const input = [
      "db.logs.find({ message: /warn;retry/i }); // keep ; here",
      "const label = `a;b`;",
      "/* block ; comment */ db.logs.countDocuments({});",
    ].join("\n");

    expect(splitNativeCliCommands(input, "mongosh").map((item) => item.text)).toEqual([
      "db.logs.find({ message: /warn;retry/i });",
      "// keep ; here\nconst label = `a;b`;",
      "/* block ; comment */ db.logs.countDocuments({});",
    ]);
  });

  it("supports newline-terminated mongosh commands without semicolons", () => {
    const input = "show dbs\nuse analytics\ndb.events.findOne({ kind: 'login' })";
    expect(splitNativeCliCommands(input, "mongosh").map((item) => item.text)).toEqual([
      "show dbs",
      "use analytics",
      "db.events.findOne({ kind: 'login' })",
    ]);
  });

  it("finds the command under the cursor using source ranges", () => {
    const input = "PING\nGET alpha\nGET beta";
    const commands = splitNativeCliCommands(input, "redis-cli");
    expect(findNativeCliCommandAtOffset(commands, input.indexOf("alpha"))?.text).toBe(
      "GET alpha",
    );
    expect(findNativeCliCommandAtOffset(commands, input.indexOf("beta"))?.text).toBe(
      "GET beta",
    );
  });
});
