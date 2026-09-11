import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  applyEnv,
  getEnv,
  getEnvDiff,
  putEnvDraft,
  type EnvChange,
  type EnvVar,
} from "../api/client";

/** Serves: env (spec Appendix D — .env editor + guarded apply). */
export const FEATURE_IDS = ["env"];

/** Pure: fold per-row edits into a draft values map (unit-testable). */
export function foldDraft(
  edits: Record<string, string>,
  remove: string[],
): { values: Record<string, string>; remove: string[] } {
  const values: Record<string, string> = {};
  for (const [key, value] of Object.entries(edits)) {
    const trimmed = value.trim();
    if (trimmed !== "" && !remove.includes(key)) values[key] = trimmed;
  }
  return { values, remove: [...remove] };
}

/** Pure: summarize a diff for the banner (unit-testable). */
export function diffSummary(changes: EnvChange[]): {
  counts: Record<EnvChange["kind"], number>;
  consumers: string[];
} {
  const counts: Record<EnvChange["kind"], number> = { added: 0, changed: 0, removed: 0 };
  const consumers = new Set<string>();
  for (const change of changes) {
    counts[change.kind] += 1;
    change.consumers.forEach((c) => consumers.add(c));
  }
  return { counts, consumers: [...consumers].sort() };
}

function VarRow({
  varInfo,
  draftValue,
  onEdit,
  onCancel,
}: {
  varInfo: EnvVar;
  draftValue: string;
  onEdit: (value: string) => void;
  onCancel: () => void;
}) {
  const editing = draftValue !== undefined;
  return (
    <div className="cd-env-var">
      <div className="cd-env-var-meta">
        <code>{varInfo.key}</code>
        {varInfo.secret && <span className="cd-tag cd-tag-secret">secret</span>}
        {varInfo.stale && <span className="cd-tag cd-tag-stale">stale</span>}
        {!varInfo.set && <span className="cd-tag cd-tag-unset">unset</span>}
        {editing && <span className="cd-tag cd-tag-edit">edited</span>}
        {varInfo.doc && <span className="cd-env-doc">{varInfo.doc}</span>}
      </div>
      <div className="cd-env-var-value">
        {editing ? (
          <input
            aria-label={`Edit ${varInfo.key}`}
            value={draftValue}
            onChange={(e) => onEdit(e.target.value)}
            placeholder={varInfo.set ? "new value…" : "set a value…"}
          />
        ) : (
          <span className="cd-env-masked">{varInfo.masked || "—"}</span>
        )}
      </div>
      <div className="cd-env-var-actions">
        {editing ? (
          <button type="button" onClick={onCancel} className="cd-btn-quiet" aria-label={`Cancel edit ${varInfo.key}`}>
            Cancel
          </button>
        ) : (
          <button type="button" onClick={() => onEdit("")} className="cd-btn-quiet" aria-label={`Edit ${varInfo.key}`}>
            Edit
          </button>
        )}
      </div>
    </div>
  );
}

function DiffRow({ change }: { change: EnvChange }) {
  return (
    <div className="cd-env-diff-row">
      <div className="cd-env-diff-head">
        <code>{change.key}</code>
        <span className={`cd-tag cd-tag-kind cd-tag-${change.kind}`}>{change.kind}</span>
        <span className="cd-env-consumers">→ {change.consumers.join(", ")}</span>
      </div>
      <div className="cd-env-diff-values">
        {change.old !== undefined && (
          <span className="cd-env-diff-old">- {change.old === "" ? "(empty)" : change.old}</span>
        )}
        {change.new !== undefined && (
          <span className="cd-env-diff-new">+ {change.new === "" ? "(empty)" : change.new}</span>
        )}
      </div>
    </div>
  );
}

export default function Env() {
  const queryClient = useQueryClient();
  const env = useQuery({ queryKey: ["env"], queryFn: getEnv, refetchInterval: 30_000 });
  const diff = useQuery({ queryKey: ["env", "diff"], queryFn: getEnvDiff, refetchInterval: 15_000 });
  const [edits, setEdits] = useState<Record<string, string>>({});
  const [removals, setRemovals] = useState<string[]>([]);
  const [confirm, setConfirm] = useState("");
  const [applied, setApplied] = useState<string | null>(null);
  const [applyError, setApplyError] = useState<string | null>(null);

  const draft = useMemo(() => foldDraft(edits, removals), [edits, removals]);
  const hasLocalEdits =
    Object.keys(draft.values).length > 0 || draft.remove.length > 0;
  const hasStagedDraft = (diff.data?.changes.length ?? 0) > 0;

  const stage = useMutation({
    mutationFn: () => putEnvDraft(draft.values, draft.remove),
    onSuccess: async (result) => {
      await queryClient.invalidateQueries({ queryKey: ["env", "diff"] });
      if (!result.staged && result.blocking) {
        const first = result.issues.find((i) => i.severity === "error");
        setApplyError(first ? `Blocked: ${first.message} (${first.key})` : "Draft blocked by validation.");
      }
    },
    onError: (error) => setApplyError(String(error)),
  });

  const apply = useMutation({
    mutationFn: () => applyEnv(confirm.trim()),
    onSuccess: async (result) => {
      setApplied(`Applied ${result.changes.length} change(s) — job ${result.jobId} · backup ${result.backup}`);
      setEdits({});
      setRemovals([]);
      setConfirm("");
      setApplyError(null);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["env"] }),
        queryClient.invalidateQueries({ queryKey: ["env", "diff"] }),
      ]);
    },
    onError: (error) => setApplyError(String(error)),
  });

  const summary = useMemo(() => diffSummary(diff.data?.changes ?? []), [diff.data]);

  return (
    <div>
      <div className="cd-topbar">
        <span className="cd-breadcrumb">Cave Deck <span>/</span> Env</span>
        <span className="cd-muted">{env.data?.path ?? ""}</span>
      </div>

      <section className="cd-page-heading">
        <div>
          <span className="cd-kicker">Configuration / .env engine</span>
          <h1>Environment editor</h1>
          <p>View grouped and masked. Edit stages a draft; apply is the guarded cascade (landmine #4 when nzbdav is touched).</p>
        </div>
        <div className="cd-heading-meta">
          <span className="cd-live-pill"><span className="cd-live-dot" /> {env.data ? `${env.data.sections.length} groups` : "loading"}</span>
        </div>
      </section>

      {env.data?.note && <div className="cd-banner">{env.data.note}</div>}

      {env.isLoading && <div className="cd-loading-state"><span className="cd-spinner" /> Loading .env…</div>}
      {env.isError && <div className="cd-error-state"><strong>Env unavailable.</strong> {String(env.error)}<button onClick={() => env.refetch()}>Retry</button></div>}

      {/* Draft banner — shown when local edits exist OR a staged backend draft does */}
      {(hasLocalEdits || hasStagedDraft) && (
        <div className="cd-card cd-draft-card">
          <div className="cd-panel-heading">
            <div>
              <span className="cd-eyebrow">Staged draft</span>
              <h2>{summary.counts.changed} changed · {summary.counts.added} added · {summary.counts.removed} removed</h2>
            </div>
            <span className="cd-panel-mark">✎</span>
          </div>
          <p className="cd-muted">
            Blast radius: <strong>{summary.consumers.join(", ") || "none"}</strong> · apply writes atomically with a backup.
          </p>
          {hasLocalEdits && !hasStagedDraft && (
            <div className="cd-banner">
              Local edits pending — stage them to preview the real diff and enable apply.
            </div>
          )}
          {diff.isError && <div className="cd-error-state"><strong>Diff unavailable.</strong> {String(diff.error)}</div>}
          {diff.data?.changes.map((change) => <DiffRow key={`${change.kind}-${change.key}`} change={change} />)}
          <div className="cd-apply-bar">
            <button
              type="button"
              className="cd-btn-primary"
              disabled={!hasLocalEdits || stage.isPending}
              onClick={() => stage.mutate()}
            >
              {stage.isPending ? "Staging…" : hasStagedDraft ? "Re-stage draft" : "Stage draft"}
            </button>
            <input
              aria-label="Confirm apply"
              placeholder="type 'env' to confirm"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
            />
            <button
              type="button"
              className="cd-btn-primary"
              disabled={confirm.trim() !== "env" || !hasStagedDraft || apply.isPending}
              onClick={() => apply.mutate()}
            >
              {apply.isPending ? "Applying…" : "Apply .env changes"}
            </button>
            <button
              type="button"
              className="cd-btn-quiet"
              onClick={() => { setEdits({}); setRemovals([]); setConfirm(""); setApplied(null); }}
            >
              Discard draft
            </button>
          </div>
          {apply.isPending && <div className="cd-muted"><span className="cd-spinner" /> Guard check + atomic write…</div>}
          {applied && <div className="cd-banner" style={{ borderColor: "#4dd69a55", color: "var(--cd-green)" }}>{applied}</div>}
          {applyError && <div className="cd-error-state"><strong>Apply refused.</strong> {applyError}</div>}
        </div>
      )}

      {/* Groups */}
      {env.data?.sections.map((section) => (
        <section key={section.name} className="cd-card cd-env-section">
          <div className="cd-panel-heading">
            <div>
              <span className="cd-eyebrow">Group</span>
              <h2>{section.name}</h2>
            </div>
            <span className="cd-panel-mark">⚙</span>
          </div>
          <div className="cd-env-vars">
            {section.vars.map((varInfo) => (
              <VarRow
                key={varInfo.key}
                varInfo={varInfo}
                draftValue={edits[varInfo.key]}
                onEdit={(value) => {
                  setEdits((prev) => ({ ...prev, [varInfo.key]: value }));
                  setRemovals((prev) => prev.filter((k) => k !== varInfo.key));
                }}
                onCancel={() => {
                  setEdits((prev) => {
                    const next = { ...prev };
                    delete next[varInfo.key];
                    return next;
                  });
                }}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}