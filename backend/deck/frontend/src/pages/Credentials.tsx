import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteProvider,
  getCredentials,
  getPlexToken,
  getProviders,
  revealKey,
  rotateKey,
  setPlexToken,
  testProvider,
  upsertProvider,
  type CredentialGroup,
  type EnvVar,
  type ProviderSlot,
} from "../api/client";

/** Serves: cred.indexers (spec Appendix D). Reveal/rotate/providers are the
 * M2 credentials-panel surface served from the same area router. */
export const FEATURE_IDS = ["cred.indexers"];

/** Pure: pick the revealable secret keys from the grouped view (unit-testable). */
export function secretKeys(groups: CredentialGroup[]): string[] {
  return groups
    .flatMap((group) => group.vars)
    .filter((v) => v.secret)
    .map((v) => v.key)
    .sort();
}

/** Pure: format a provider's connection line (unit-testable). */
export function providerConnection(slot: ProviderSlot): string {
  if (!slot.host && !slot.user) return "not configured";
  return `${slot.user ?? "(no user)"}@${slot.host ?? "(no host)"}:${slot.port ?? "?"}${slot.passSet ? " · pass set" : " · no pass"}`;
}

/** Reveal state: which key is revealed and the plaintext (auto-remasks 15s). */
export function useReveal() {
  const [revealed, setRevealed] = useState<{ key: string; value: string } | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, []);
  const reveal = (key: string, value: string) => {
    setRevealed({ key, value });
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => setRevealed(null), 15_000);
  };
  return { revealed, reveal };
}

const ROTATABLE: Array<{ key: string; label: string }> = [
  { key: "RADARR_API_KEY", label: "Radarr" },
  { key: "SONARR_API_KEY", label: "Sonarr" },
  { key: "PROWLARR_API_KEY", label: "Prowlarr" },
  { key: "SEERR_API_KEY", label: "Seerr" },
];

function VarLine({
  varInfo,
  revealed,
  onReveal,
}: {
  varInfo: EnvVar;
  revealed: { key: string; value: string } | null;
  onReveal: (key: string, value: string) => void;
}) {
  const isRevealed = revealed?.key === varInfo.key;
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const doReveal = async () => {
    setBusy(true);
    setError(null);
    try {
      const result = await revealKey(varInfo.key);
      onReveal(result.key, result.value);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="cd-env-var">
      <div className="cd-env-var-meta">
        <code>{varInfo.key}</code>
        {varInfo.stale && <span className="cd-tag cd-tag-stale">stale</span>}
        {!varInfo.set && <span className="cd-tag cd-tag-unset">unset</span>}
        {isRevealed && <span className="cd-tag cd-tag-revealed">revealed · re-masks in 15s</span>}
      </div>
      <div className="cd-env-var-value">
        {isRevealed ? (
          <code className="cd-revealed-value">{revealed?.value}</code>
        ) : (
          <span className="cd-env-masked">{varInfo.masked || "—"}</span>
        )}
      </div>
      <div className="cd-env-var-actions">
        <button type="button" className="cd-btn-quiet" disabled={busy || isRevealed} onClick={doReveal}>
          {isRevealed ? "Revealed" : busy ? "…" : "Reveal"}
        </button>
      </div>
      {error && <div className="cd-error-state cd-inline-error">{error}</div>}
    </div>
  );
}

function RotateRow({ keyName, label, onResult }: { keyName: string; label: string; onResult: (msg: string) => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const doRotate = async () => {
    if (!window.confirm(`Rotate ${label}'s API key? The new key is pushed to the app and staged into the .env draft.`)) return;
    setBusy(true);
    setError(null);
    try {
      const result = await rotateKey(keyName);
      onResult(
        result.rotated
          ? `${label} rotated. New key staged — apply via the Env page (queue guard runs).`
          : `${label} rotation failed on the app side: ${result.pushError ?? "unknown error"} (still staged for review).`,
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="cd-rotate-row">
      <div className="cd-env-var-meta">
        <code>{keyName}</code>
        <span className="cd-muted">{label}</span>
      </div>
      <div className="cd-env-var-actions">
        <button type="button" disabled={busy} onClick={doRotate}>{busy ? "Rotating…" : "Rotate key"}</button>
      </div>
      {error && <div className="cd-error-state cd-inline-error">{error}</div>}
    </div>
  );
}

function ProviderCard({
  slot,
  onMessage,
}: {
  slot: ProviderSlot;
  onMessage: (msg: string, tone?: "ok" | "err") => void;
}) {
  const [editing, setEditing] = useState(false);
  const [host, setHost] = useState(slot.host ?? "");
  const [port, setPort] = useState(slot.port ? String(slot.port) : "");
  const [user, setUser] = useState(slot.user ?? "");
  const [pass, setPass] = useState("");
  const [busy, setBusy] = useState(false);
  const queryClient = useQueryClient();

  const save = async () => {
    setBusy(true);
    try {
      const body: Record<string, string | number> = { host: host.trim() || (slot.host ?? ""), user: user.trim() || (slot.user ?? "") };
      if (port.trim() !== "") body.port = Number(port);
      if (pass !== "") body.pass = pass;
      const result = await upsertProvider(slot.nickname, body);
      if (result.blocking) {
        onMessage(`Blocked: ${result.issues.map((i) => i.message).join("; ")}`, "err");
      } else {
        onMessage(`Provider '${slot.nickname}' staged — apply via the Env page.`);
        setEditing(false);
        setPass("");
      }
      await queryClient.invalidateQueries({ queryKey: ["providers"] });
    } catch (e) {
      onMessage(String(e), "err");
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    if (!window.confirm(`Disable provider '${slot.nickname}'? Its vars are staged for removal.`)) return;
    setBusy(true);
    try {
      await deleteProvider(slot.nickname);
      onMessage(`Provider '${slot.nickname}' removal staged — apply via the Env page.`);
      await queryClient.invalidateQueries({ queryKey: ["providers"] });
    } catch (e) {
      onMessage(String(e), "err");
    } finally {
      setBusy(false);
    }
  };

  const runTest = async () => {
    setBusy(true);
    try {
      const result = await testProvider({
        host: host.trim() || (slot.host ?? ""),
        port: port.trim() !== "" ? Number(port) : slot.port,
        user: user.trim() || (slot.user ?? ""),
        pass: pass || undefined,
      });
      onMessage(
        result.connected
          ? `'${slot.nickname}': connected (HTTP ${result.status})`
          : `'${slot.nickname}': NOT connected (${result.error ?? `HTTP ${result.status}`})`,
        result.connected ? "ok" : "err",
      );
    } catch (e) {
      onMessage(String(e), "err");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="cd-card cd-provider-card">
      <div className="cd-panel-heading">
        <div>
          <span className="cd-eyebrow">Provider slot</span>
          <h2>{slot.nickname}</h2>
        </div>
        <div className="cd-provider-tags">
          {slot.wired && <span className="cd-tag cd-tag-wired">compose-wired</span>}
          {slot.enabled ? <span className="cd-tag cd-tag-ok">enabled</span> : <span className="cd-tag cd-tag-unset">dormant</span>}
        </div>
      </div>
      <p className="cd-muted">{providerConnection(slot)} · {slot.passMasked || "no pass set"}</p>
      {editing && (
        <div className="cd-provider-form">
          <div className="cd-provider-fields">
            <label>Host<input aria-label="host" value={host} onChange={(e) => setHost(e.target.value)} /></label>
            <label>Port<input aria-label="port" value={port} onChange={(e) => setPort(e.target.value)} /></label>
            <label>User<input aria-label="user" value={user} onChange={(e) => setUser(e.target.value)} /></label>
            <label>Pass<input aria-label="pass" type="password" value={pass} onChange={(e) => setPass(e.target.value)} placeholder="unchanged" /></label>
          </div>
          <div className="cd-env-var-actions">
            <button type="button" disabled={busy} onClick={save}>Save (stages draft)</button>
            <button type="button" disabled={busy} onClick={runTest}>Test connection</button>
            {!slot.wired && (
              <button type="button" className="cd-btn-danger" disabled={busy} onClick={remove}>Disable</button>
            )}
            <button type="button" className="cd-btn-quiet" disabled={busy} onClick={() => setEditing(false)}>Cancel</button>
          </div>
        </div>
      )}
      {!editing && (
        <div className="cd-env-var-actions">
          <button type="button" onClick={() => setEditing(true)}>Edit / Test</button>
        </div>
      )}
    </div>
  );
}

function PlexTokenCard({ onMessage }: { onMessage: (msg: string, tone?: "ok" | "err") => void }) {
  const plex = useQuery({ queryKey: ["plex", "token"], queryFn: getPlexToken, refetchInterval: 30_000 });
  const [token, setToken] = useState("");
  const [busy, setBusy] = useState(false);

  const save = async () => {
    setBusy(true);
    try {
      await setPlexToken(token);
      onMessage(`PLEX_TOKEN staged — apply via the Env page.`);
      setToken("");
      plex.refetch();
    } catch (e) {
      onMessage(String(e), "err");
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="cd-card">
      <div className="cd-panel-heading">
        <div>
          <span className="cd-eyebrow">Plex</span>
          <h2>Server token</h2>
        </div>
        <span className="cd-panel-mark">●</span>
      </div>
      {plex.isLoading && <p className="cd-muted">Verifying…</p>}
      {plex.data && (
        <p className="cd-muted">
          {plex.data.tokenSet ? "Token configured" : "No token set"} ·{" "}
          {plex.data.reachable ? (plex.data.valid ? <span style={{ color: "var(--cd-green)" }}>valid</span> : <span style={{ color: "var(--cd-orange)" }}>invalid</span>) : `unreachable (${plex.data.error ?? ""})`}
          {plex.data.status ? ` · HTTP ${plex.data.status}` : ""}
        </p>
      )}
      {plex.isError && <div className="cd-error-state cd-inline-error">{String(plex.error)}</div>}
      <div className="cd-provider-fields">
        <label>New PLEX_TOKEN<input aria-label="plex token" type="password" value={token} onChange={(e) => setToken(e.target.value)} placeholder="paste new token…" /></label>
      </div>
      <div className="cd-env-var-actions">
        <button type="button" disabled={busy || token.trim() === ""} onClick={save}>{busy ? "Staging…" : "Stage token"}</button>
      </div>
    </div>
  );
}

export default function Credentials() {
  const creds = useQuery({ queryKey: ["credentials"], queryFn: getCredentials, refetchInterval: 30_000 });
  const providers = useQuery({ queryKey: ["providers"], queryFn: getProviders, refetchInterval: 30_000 });
  const { revealed, reveal } = useReveal();
  const [message, setMessage] = useState<{ text: string; tone: "ok" | "err" } | null>(null);

  const onMessage = (text: string, tone: "ok" | "err" = "ok") => setMessage({ text, tone });

  return (
    <div>
      <div className="cd-topbar">
        <span className="cd-breadcrumb">Cave Deck <span>/</span> Credentials</span>
        <span className="cd-muted">{creds.data ? `${secretKeys(creds.data.groups).length} secret keys` : ""}</span>
      </div>

      <section className="cd-page-heading">
        <div>
          <span className="cd-kicker">Credentials / API sources</span>
          <h1>Credentials panel</h1>
          <p>Values masked by default; reveal on click (audit-logged, auto re-mask 15s). Rotation pushes to the app, then stages the .env draft.</p>
        </div>
        <div className="cd-heading-meta">
          <span className="cd-live-pill"><span className="cd-live-dot" /> M2 · reveal + rotation live</span>
        </div>
      </section>

      {message && (
        <div className={`cd-banner ${message.tone === "err" ? "cd-banner-err" : ""}`} role="status">
          {message.text}
        </div>
      )}

      {creds.isLoading && <div className="cd-loading-state"><span className="cd-spinner" /> Loading credentials…</div>}
      {creds.isError && <div className="cd-error-state"><strong>Credentials unavailable.</strong> {String(creds.error)}<button onClick={() => creds.refetch()}>Retry</button></div>}

      {/* Secret keys — grouped masked with reveal */}
      {creds.data?.groups.map((group) => (
        <section key={group.name} className="cd-card cd-env-section">
          <div className="cd-panel-heading">
            <div>
              <span className="cd-eyebrow">Group</span>
              <h2>{group.name}</h2>
            </div>
            <span className="cd-panel-mark">▣</span>
          </div>
          <div className="cd-env-vars">
            {group.vars.map((varInfo) => (
              <VarLine key={varInfo.key} varInfo={varInfo} revealed={revealed} onReveal={(key, value) => reveal(key, value)} />
            ))}
          </div>
        </section>
      ))}

      {/* Rotation */}
      <section className="cd-card cd-env-section">
        <div className="cd-panel-heading">
          <div>
            <span className="cd-eyebrow">Key rotation</span>
            <h2>Rotate app API keys</h2>
          </div>
          <span className="cd-panel-mark">⇄</span>
        </div>
        <p className="cd-muted">New key pushed to the app (config/host for *arr, regenerate for Seerr), then staged into the .env draft for the guarded apply.</p>
        <div className="cd-env-vars">
          {ROTATABLE.map(({ key, label }) => (
            <RotateRow key={key} keyName={key} label={label} onResult={(msg) => onMessage(msg)} />
          ))}
        </div>
      </section>

      {/* Usenet providers */}
      <section className="cd-card cd-env-section">
        <div className="cd-panel-heading">
          <div>
            <span className="cd-eyebrow">Usenet providers</span>
            <h2>Provider slots</h2>
          </div>
          <span className="cd-panel-mark">⇩</span>
        </div>
        <p className="cd-muted">Backed by the flat NZBDAV_USENET_* vars. Wired slots are compose-referenced; dormant slots can be disabled. Edits stage the draft.</p>
        {providers.isLoading && <div className="cd-loading-state"><span className="cd-spinner" /> Loading providers…</div>}
        {providers.isError && <div className="cd-error-state"><strong>Providers unavailable.</strong> {String(providers.error)}<button onClick={() => providers.refetch()}>Retry</button></div>}
        <div className="cd-provider-grid">
          {providers.data?.providers.map((slot) => (
            <ProviderCard key={slot.nickname} slot={slot} onMessage={onMessage} />
          ))}
        </div>
      </section>

      {/* Plex token */}
      <PlexTokenCard onMessage={onMessage} />
    </div>
  );
}