import { describe, it, expect } from "vitest";
import { secretKeys, providerConnection } from "../src/pages/Credentials";
import type { CredentialGroup, ProviderSlot } from "../src/api/client";

const GROUPS: CredentialGroup[] = [
  {
    name: "*arr API Keys",
    vars: [
      { key: "RADARR_API_KEY", secret: true, stale: false, set: true, masked: "•••• (32 chars)" },
      { key: "RADARR_URL", secret: false, stale: false, set: true, masked: "" },
      { key: "SONARR_API_KEY", secret: true, stale: true, set: true, masked: "•••• (8 chars)" },
    ],
  },
  {
    name: "Identity / Runtime",
    vars: [
      { key: "PUID", secret: false, stale: false, set: true, masked: "" },
      { key: "PLEX_TOKEN", secret: true, stale: false, set: false, masked: "" },
    ],
  },
];

describe("secretKeys", () => {
  it("returns only secret keys, sorted", () => {
    expect(secretKeys(GROUPS)).toEqual(["PLEX_TOKEN", "RADARR_API_KEY", "SONARR_API_KEY"]);
  });
});

describe("providerConnection", () => {
  const base: ProviderSlot = {
    nickname: "primary",
    wired: true,
    enabled: true,
    host: "usenet.example.com",
    port: 563,
    user: "alice",
    passMasked: "•••• (16 chars)",
    passSet: true,
    consumers: ["nzbdav"],
  };

  it("formats a configured slot", () => {
    expect(providerConnection(base)).toContain("alice@usenet.example.com:563");
    expect(providerConnection(base)).toContain("pass set");
  });

  it("reports an unconfigured slot", () => {
    const empty: ProviderSlot = { ...base, host: undefined, user: undefined, passSet: false, passMasked: "" };
    expect(providerConnection(empty)).toBe("not configured");
  });

  it("flags a missing pass", () => {
    const noPass: ProviderSlot = { ...base, passSet: false, passMasked: "" };
    expect(providerConnection(noPass)).toContain("no pass");
  });
});