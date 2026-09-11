import { describe, it, expect } from "vitest";
import { foldDraft, diffSummary } from "../src/pages/Env";
import type { EnvChange } from "../src/api/client";

describe("foldDraft", () => {
  it("keeps only non-empty trimmed values not scheduled for removal", () => {
    const { values, remove } = foldDraft(
      { PLEX_URL: " http://10.0.0.9:32400 ", EMPTY: "", REMOVED: "x" },
      ["REMOVED"],
    );
    expect(values).toEqual({ PLEX_URL: "http://10.0.0.9:32400" });
    expect(remove).toEqual(["REMOVED"]);
  });

  it("copies the remove list (no aliasing)", () => {
    const remove = ["A"];
    const result = foldDraft({}, remove);
    expect(result.remove).toEqual(["A"]);
    expect(result.remove).not.toBe(remove);
  });
});

describe("diffSummary", () => {
  const changes: EnvChange[] = [
    { key: "PLEX_URL", section: "Plex", kind: "changed", old: "a", new: "b", consumers: ["plex", "cave-deck"] },
    { key: "SEERR_API_KEY", section: "Seerr", kind: "added", old: undefined, new: "k", consumers: ["seerr", "cave-deck"] },
    { key: "OLD", section: "General", kind: "removed", old: "v", new: undefined, consumers: ["cave-deck"] },
    { key: "PLEX_TOKEN", section: "Plex", kind: "changed", old: "x", new: "y", consumers: ["plex", "cave-deck"] },
  ];

  it("counts by kind and dedupes consumers", () => {
    const { counts, consumers } = diffSummary(changes);
    expect(counts).toEqual({ added: 1, changed: 2, removed: 1 });
    expect(consumers).toEqual(["cave-deck", "plex", "seerr"]);
  });

  it("handles an empty diff", () => {
    const { counts, consumers } = diffSummary([]);
    expect(counts).toEqual({ added: 0, changed: 0, removed: 0 });
    expect(consumers).toEqual([]);
  });
});