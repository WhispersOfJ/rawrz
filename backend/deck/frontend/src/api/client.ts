const BASE = "/api/v1";

export interface Job {
  id: string;
  kind: string;
  target: string;
  state: "queued" | "running" | "done" | "failed" | "cancelled";
}

export async function api<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { "content-type": "application/json", ...init?.headers },
    ...init,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => null);
    throw new Error(body?.error?.message ?? `${res.status} ${res.statusText}`);
  }
  return res.json() as Promise<T>;
}

export const getVersion = () => api<{ cave_deck: string; stack: string }>("/version");
export const getCatalog = () =>
  api<CatalogDocument>("/catalog");

export interface CatalogEnvVar {
  key: string;
  default: string | null;
  secret?: boolean;
}

export interface CatalogEntry {
  id: string;
  name: string;
  category: string;
  retired: boolean;
  image: string;
  ports: string[];
  volumes: string[];
  env: CatalogEnvVar[];
  pid_mode?: string;
  host_network?: boolean;
  devices: string[];
  capabilities: string[];
  dependencies: string[];
  compose_fragment?: string;
  mem_limit: string;
  healthcheck?: string;
  docs: string;
  notes: string;
}

export interface CatalogDocument {
  version: string;
  updated: string;
  installState: string;
  entries: CatalogEntry[];
}
export interface DashboardSnapshot {
  containers: Array<{
    id: string;
    status: string;
    health: string;
  }>;
  mount: { healthy: boolean; probed: boolean };
  queues: { sonarr: number | null; radarr: number | null; nzbdav: number | null };
  disk: { free_bytes: number | null };
  note: string;
}

export const getDashboard = () => api<DashboardSnapshot>("/dashboard");
export const getJobs = () => api<{ jobs: Job[] }>("/jobs");

// --- .env engine (M2 env editor, spec §6.5) ---

export interface EnvVar {
  key: string;
  doc?: string;
  secret: boolean;
  stale: boolean;
  set: boolean;
  masked: string;
}

export interface EnvSection {
  name: string;
  vars: EnvVar[];
}

export interface EnvView {
  sections: EnvSection[];
  note?: string;
  path: string;
}

export interface EnvIssue {
  key: string;
  code: string;
  message: string;
  severity: "error" | "warning" | "info";
}

export interface EnvDraftResult {
  staged: boolean;
  issues: EnvIssue[];
  blocking: boolean;
}

export interface EnvChange {
  key: string;
  section: string;
  kind: "added" | "changed" | "removed";
  old?: string;
  new?: string;
  consumers: string[];
}

export interface EnvApplyResult {
  jobId: string;
  backup: string;
  changes: EnvChange[];
  consumers: string[];
  note: string;
}

export const getEnv = () => api<EnvView>("/env");
export const putEnvDraft = (values: Record<string, string>, remove: string[]) =>
  api<EnvDraftResult>("/env/draft", {
    method: "PUT",
    body: JSON.stringify({ values, remove }),
  });
export const getEnvDiff = () => api<{ changes: EnvChange[] }>("/env/diff");
export const getEnvConsumers = (key: string) =>
  api<{ var: string; consumers: string[] }>(`/env/consumers/${encodeURIComponent(key)}`);
export const applyEnv = (confirm: string) =>
  api<EnvApplyResult>("/env/apply", {
    method: "POST",
    body: JSON.stringify({ confirm }),
  });

// --- Credentials & API sources (M2 credentials panel, spec §6.4) ---

export interface CredentialGroup {
  name: string;
  vars: EnvVar[];
}

export interface RevealResult {
  key: string;
  value: string;
  audited: boolean;
}

export interface RotationResult {
  key: string;
  app: string;
  rotated: boolean;
  staged: boolean;
  consumers: string[];
  pushError?: string;
  note: string;
}

export interface ProviderSlot {
  nickname: string;
  wired: boolean;
  enabled: boolean;
  host?: string;
  port?: number;
  user?: string;
  passMasked: string;
  passSet: boolean;
  consumers: string[];
}

export interface ProviderUpsertResult {
  staged: boolean;
  blocking: boolean;
  issues: EnvIssue[];
  changes: EnvChange[];
  consumers: string[];
  note?: string;
}

export interface ProviderTestResult {
  reachable: boolean;
  status?: number;
  connected: boolean;
  payload?: unknown;
  error?: string;
}

export interface PlexTokenState {
  url: string;
  tokenSet: boolean;
  valid: boolean;
  reachable: boolean;
  status?: number;
  error?: string;
}

export const getCredentials = () => api<{ groups: CredentialGroup[] }>("/credentials");
export const revealKey = (key: string) =>
  api<RevealResult>(`/credentials/${encodeURIComponent(key)}/reveal`, { method: "POST" });
export const rotateKey = (key: string) =>
  api<RotationResult>(`/credentials/${encodeURIComponent(key)}`, { method: "POST" });
export const getProviders = () => api<{ providers: ProviderSlot[] }>("/usenet/providers");
export const upsertProvider = (nickname: string, body: Record<string, string | number>) =>
  api<ProviderUpsertResult>(`/usenet/providers/${encodeURIComponent(nickname)}`, {
    method: "POST",
    body: JSON.stringify(body),
  });
export const deleteProvider = (nickname: string) =>
  api<{ staged: boolean; removed: string[]; consumers: string[]; note: string }>(
    `/usenet/providers/${encodeURIComponent(nickname)}`,
    { method: "DELETE" },
  );
export const testProvider = (body: {
  host: string;
  port?: number;
  user: string;
  pass?: string;
  useSsl?: boolean;
}) =>
  api<ProviderTestResult>("/usenet/providers/test", {
    method: "POST",
    body: JSON.stringify(body),
  });
export const getPlexToken = () => api<PlexTokenState>("/plex/token");
export const setPlexToken = (token: string) =>
  api<{ staged: boolean; key: string; set: boolean; consumers: string[]; note: string }>(
    "/plex/token",
    { method: "POST", body: JSON.stringify({ token }) },
  );
