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
export const getDashboard = () =>
  api<{
    containers: unknown[];
    mount: { healthy: boolean; probed: boolean };
    queues: { sonarr: number | null; radarr: number | null; nzbdav: number | null };
    disk: { free_bytes: number | null };
    note: string;
  }>("/dashboard");
export const getJobs = () => api<{ jobs: Job[] }>("/jobs");
