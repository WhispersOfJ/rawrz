import { useQuery } from "@tanstack/react-query";
import { getDashboard, getVersion } from "../api/client";

/** Serves: dash.overview, dash.rows (spec Appendix D). */
export default function Dashboard() {
  const dash = useQuery({ queryKey: ["dashboard"], queryFn: getDashboard });
  const ver = useQuery({ queryKey: ["version"], queryFn: getVersion });

  return (
    <div>
      <h1>Dashboard</h1>
      <div className="cd-banner">
        M0 skeleton — data plumbing lands in M1. Backend: {ver.data?.cave_deck ?? "…"}
      </div>
      <div className="cd-card">
        <h2>Stack snapshot</h2>
        {dash.isLoading && <p>Loading…</p>}
        {dash.data && (
          <ul>
            <li>Containers: {dash.data.containers.length}</li>
            <li>
              Mount:{" "}
              {dash.data.mount.probed
                ? dash.data.mount.healthy ? "healthy" : "DEGRADED"
                : "not probed (M1)"}
            </li>
            <li>Queues: sonarr {dash.data.queues.sonarr ?? "—"} · radarr {dash.data.queues.radarr ?? "—"} · nzbdav {dash.data.queues.nzbdav ?? "—"}</li>
            <li>Disk free: {dash.data.disk.free_bytes ?? "—"}</li>
          </ul>
        )}
      </div>
    </div>
  );
}
