import { describe, it, expect } from "vitest";
import { filterEntries, type Category } from "../src/pages/Catalog";
import type { CatalogEntry } from "../src/api/client";

function entry(over: Partial<CatalogEntry>): CatalogEntry {
  return {
    id: "test",
    name: "Test",
    category: "ops",
    retired: false,
    image: "img:1",
    ports: [],
    volumes: [],
    env: [],
    devices: [],
    capabilities: [],
    dependencies: [],
    mem_limit: "256m",
    docs: "https://example.com",
    notes: "",
    ...over,
  };
}

const CATALOG: CatalogEntry[] = [
  entry({ id: "glances", name: "Glances", category: "ops", image: "nicolargo/glances", notes: "host stats" }),
  entry({ id: "sonarr", name: "Sonarr", category: "media", image: "lscr.io/linuxserver/sonarr", notes: "tv" }),
  entry({ id: "caddy", name: "Caddy", category: "network", image: "caddy:2", notes: "reverse proxy" }),
];

const CATEGORIES: Category[] = ["all", "media", "ops", "network", "home"];

describe("catalog filtering", () => {
  it("has coherent category constants", () => {
    expect(CATEGORIES).toContain("all");
  });

  it("filters by category", () => {
    expect(filterEntries(CATALOG, "media", "").map((e) => e.id)).toEqual(["sonarr"]);
    expect(filterEntries(CATALOG, "all", "")).toHaveLength(3);
  });

  it("searches across name, id, image and notes, case-insensitively", () => {
    expect(filterEntries(CATALOG, "all", "sonarr").map((e) => e.id)).toEqual(["sonarr"]);
    expect(filterEntries(CATALOG, "all", "GLANCES").map((e) => e.id)).toEqual(["glances"]);
    expect(filterEntries(CATALOG, "all", "linuxserver").map((e) => e.id)).toEqual(["sonarr"]);
    expect(filterEntries(CATALOG, "all", "proxy").map((e) => e.id)).toEqual(["caddy"]);
  });

  it("trims and ignores whitespace-only searches", () => {
    expect(filterEntries(CATALOG, "all", "  ")).toHaveLength(3);
  });

  it("combines category + search", () => {
    expect(filterEntries(CATALOG, "network", "caddy").map((e) => e.id)).toEqual(["caddy"]);
    expect(filterEntries(CATALOG, "media", "caddy")).toHaveLength(0);
  });
});
