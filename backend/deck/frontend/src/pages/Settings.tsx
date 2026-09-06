import { useQuery } from "@tanstack/react-query";
import { getJobs } from "../api/client";
import { BUILT_IN, applyTheme, currentTheme } from "../theme/theme";

/** Settings: theme switcher (M0) — job/notification prefs land in M2/M6. */
export default function Settings() {
  const jobs = useQuery({ queryKey: ["jobs"], queryFn: getJobs });
  const theme = currentTheme();

  return (
    <div>
      <h1>Settings</h1>
      <div className="cd-card">
        <h2>Theme</h2>
        {BUILT_IN.map((preset) => (
          <button
            key={preset.id}
            onClick={() => applyTheme(preset)}
            style={{
              marginRight: "0.5rem",
              padding: "0.5rem 1rem",
              minHeight: 44,
              border: preset.id === theme.id ? "2px solid var(--cd-accent)" : "1px solid var(--cd-border)",
              borderRadius: 8,
              background: "transparent",
              color: "var(--cd-text)",
            }}
          >
            {preset.name}
          </button>
        ))}
      </div>
      <div className="cd-card">
        <h2>Jobs</h2>
        {jobs.data?.jobs.length ? (
          <ul>
            {jobs.data.jobs.map((j) => (
              <li key={j.id}>{j.id} — {j.kind} ({j.target}) [{j.state}]</li>
            ))}
          </ul>
        ) : (
          <p>No jobs yet. Mutations spawn jobs from M3 onward.</p>
        )}
      </div>
    </div>
  );
}
