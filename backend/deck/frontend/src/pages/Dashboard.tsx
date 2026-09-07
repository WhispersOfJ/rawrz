import { useQuery } from "@tanstack/react-query";
import { getDashboard, getVersion, type DashboardSnapshot } from "../api/client";

/** Serves: dash.overview, dash.rows (spec Appendix D). */
export const FEATURE_IDS = ["dash.overview", "dash.rows"];

const SERVICES = [
  { id: "prowlarr", name: "Prowlarr", detail: "Indexer manager", port: "9696", tone: "violet" },
  { id: "radarr", name: "Radarr", detail: "Movie acquisition", port: "7878", tone: "orange" },
  { id: "sonarr", name: "Sonarr", detail: "TV acquisition", port: "8989", tone: "blue" },
  { id: "nzbdav", name: "nzbdav", detail: "Usenet + WebDAV", port: "3000", tone: "gold" },
  { id: "nzbdav_rclone", name: "rclone mount", detail: "FUSE streaming layer", port: "—", tone: "green" },
  { id: "seerr", name: "Seerr", detail: "Request management", port: "5055", tone: "pink" },
  { id: "plex", name: "Plex", detail: "Media server", port: "32400", tone: "amber" },
  { id: "unpackerr", name: "Unpackerr", detail: "Archive extraction", port: "—", tone: "cyan" },
];

function formatBytes(value: number | null): string {
  if (value === null) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let amount = value;
  let unit = 0;
  while (amount >= 1024 && unit < units.length - 1) {
    amount /= 1024;
    unit += 1;
  }
  return `${amount.toFixed(amount >= 10 || unit === 0 ? 0 : 1)} ${units[unit]}`;
}

function SnapshotCard({ label, value, detail, tone }: { label: string; value: string; detail: string; tone: string }) {
  return (
    <div className={`cd-stat-card ${tone}`}>
      <span className="cd-eyebrow">{label}</span>
      <strong>{value}</strong>
      <span className="cd-stat-detail">{detail}</span>
    </div>
  );
}

function ServiceCard({ service, snapshot }: { service: typeof SERVICES[number]; snapshot?: DashboardSnapshot["containers"][number] }) {
  const status = snapshot?.status ?? "unprobed";
  const health = snapshot?.health ?? "unknown";
  const isHealthy = status === "running" && (health === "healthy" || health === "unknown");
  return (
    <div className="cd-service-card">
      <div className={`cd-service-icon ${service.tone}`}>{service.name.slice(0, 1)}</div>
      <div className="cd-service-copy">
        <div className="cd-service-heading">
          <strong>{service.name}</strong>
          <span className={`cd-status ${isHealthy ? "ok" : status === "unprobed" ? "muted" : "warn"}`}>
            <span className="cd-status-dot" /> {status === "unprobed" ? "Not probed" : status}
          </span>
        </div>
        <span>{service.detail}</span>
        <small>Port {service.port} · {health}</small>
      </div>
      <span className="cd-chevron" aria-hidden="true">›</span>
    </div>
  );
}

function DashboardContent({ data }: { data: DashboardSnapshot }) {
  const running = data.containers.filter((container) => container.status === "running").length;
  const total = data.containers.length || SERVICES.length;
  const queueTotal = [data.queues.sonarr, data.queues.radarr, data.queues.nzbdav]
    .filter((value): value is number => value !== null)
    .reduce((sum, value) => sum + value, 0);

  return (
    <>
      <section className="cd-page-heading">
        <div>
          <span className="cd-kicker">Overview / Live state</span>
          <h1>Good morning, operator.</h1>
          <p>Your media stack at a glance. Everything important, nothing buried.</p>
        </div>
        <div className="cd-heading-meta">
          <span className="cd-live-pill"><span className="cd-live-dot" /> Live snapshot</span>
          <span className="cd-muted">Updated just now</span>
        </div>
      </section>

      <section className="cd-stat-grid" aria-label="Stack summary">
        <SnapshotCard label="Stack health" value={`${running}/${total}`} detail="services running" tone="green" />
        <SnapshotCard label="Mount status" value={data.mount.probed ? (data.mount.healthy ? "Healthy" : "Degraded") : "Unverified"} detail="FUSE streaming layer" tone={data.mount.healthy ? "blue" : "orange"} />
        <SnapshotCard label="Active queue" value={data.queues.nzbdav === null ? "—" : String(queueTotal)} detail="items across downloaders" tone="violet" />
        <SnapshotCard label="Disk available" value={formatBytes(data.disk.free_bytes)} detail="host filesystem" tone="gold" />
      </section>

      <div className="cd-dashboard-grid">
        <section className="cd-panel cd-services-panel">
          <div className="cd-panel-heading">
            <div>
              <span className="cd-eyebrow">Infrastructure</span>
              <h2>Services</h2>
            </div>
            <a href="/containers" className="cd-text-link">View all <span>→</span></a>
          </div>
          <div className="cd-service-grid">
            {SERVICES.map((service) => (
              <ServiceCard key={service.id} service={service} snapshot={data.containers.find((container) => container.id === service.id)} />
            ))}
          </div>
        </section>

        <aside className="cd-panel cd-activity-panel">
          <div className="cd-panel-heading">
            <div>
              <span className="cd-eyebrow">System signal</span>
              <h2>At a glance</h2>
            </div>
            <span className="cd-panel-mark">✦</span>
          </div>
          <div className="cd-signal-row">
            <span className="cd-signal-icon green">⌁</span>
            <div><strong>FUSE mount</strong><span>{data.mount.probed ? (data.mount.healthy ? "Streaming normally" : "Needs attention") : "Not probed"}</span></div>
          </div>
          <div className="cd-signal-row">
            <span className="cd-signal-icon violet">⇩</span>
            <div><strong>Download queue</strong><span>{data.queues.nzbdav === null ? "Not connected" : `${data.queues.nzbdav} nzbdav items`}</span></div>
          </div>
          <div className="cd-signal-row">
            <span className="cd-signal-icon orange">◷</span>
            <div><strong>Recent activity</strong><span>Imports and events surface here as they stream in</span></div>
          </div>
          <div className="cd-signal-note">{data.note}</div>
        </aside>
      </div>
    </>
  );
}

export default function Dashboard() {
  const dash = useQuery({ queryKey: ["dashboard"], queryFn: getDashboard, refetchInterval: 10_000 });
  const ver = useQuery({ queryKey: ["version"], queryFn: getVersion });

  return (
    <div>
      <div className="cd-topbar">
        <span className="cd-breadcrumb">Cave Deck <span>/</span> Dashboard</span>
        <span className="cd-version">v{ver.data?.cave_deck ?? "0.1.0"}</span>
      </div>
      <div className="cd-banner">
        Read-only control room · live probes active · mutations arrive in later milestones.
      </div>
      {dash.isLoading && <div className="cd-loading-state"><span className="cd-spinner" /> Connecting to the control room…</div>}
      {dash.isError && <div className="cd-error-state"><strong>Dashboard unavailable.</strong> {String(dash.error)}<button onClick={() => dash.refetch()}>Retry</button></div>}
      {dash.data && <DashboardContent data={dash.data} />}
      {!dash.data && !dash.isLoading && !dash.isError && <div className="cd-loading-state">No dashboard snapshot returned.</div>}
    </div>
  );
}
