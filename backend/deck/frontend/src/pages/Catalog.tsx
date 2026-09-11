import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getCatalog, type CatalogEntry } from "../api/client";

/** Serves: catalog (spec Appendix D — GET /catalog, detail + conflicts land with M4 install flow). */
export const FEATURE_IDS = ["catalog"];

const CATEGORIES = ["all", "media", "ops", "network", "home"] as const;
export type Category = (typeof CATEGORIES)[number];

/** Pure filter so the semantics are unit-testable. */
export function filterEntries(
  entries: CatalogEntry[],
  category: Category,
  search: string,
): CatalogEntry[] {
  const q = search.trim().toLowerCase();
  return entries
    .filter((e) => category === "all" || e.category === category)
    .filter(
      (e) =>
        !q ||
        `${e.name} ${e.id} ${e.image} ${e.notes}`.toLowerCase().includes(q),
    );
}

export default function Catalog() {
  const catalog = useQuery({ queryKey: ["catalog"], queryFn: getCatalog });
  const [category, setCategory] = useState<Category>("all");
  const [search, setSearch] = useState("");

  const counts = useMemo(() => {
    const all = catalog.data?.entries ?? [];
    const by = (c: string) => all.filter((e) => e.category === c).length;
    return { all: all.length, media: by("media"), ops: by("ops"), network: by("network"), home: by("home") };
  }, [catalog.data]);

  const entries = useMemo(
    () => filterEntries(catalog.data?.entries ?? [], category, search),
    [catalog.data, category, search],
  );

  return (
    <div>
      <h1>Catalog</h1>
      <div className="cd-banner">
        {catalog.data
          ? `Curated container catalog v${catalog.data.version} — ${catalog.data.entries.length} entries · updated ${catalog.data.updated} · install-state: ${catalog.data.installState}. Install/uninstall PR flows land in M4.`
          : "Loading curated catalog…"}
      </div>

      {catalog.isLoading && <div className="cd-card"><p>Loading…</p></div>}
      {catalog.isError && (
        <div className="cd-card">
          <p>Catalog unavailable: {String(catalog.error)}</p>
        </div>
      )}

      {catalog.data && (
        <>
          <div className="cd-card">
            <div role="tablist" aria-label="Catalog category filter" style={{ display: "flex", gap: "0.5rem", flexWrap: "wrap" }}>
              {CATEGORIES.map((c) => (
                <button
                  key={c}
                  role="tab"
                  aria-selected={category === c}
                  onClick={() => setCategory(c)}
                  style={category === c ? { fontWeight: 700 } : undefined}
                >
                  {c} ({counts[c]})
                </button>
              ))}
            </div>
            <input
              aria-label="Search catalog"
              placeholder="Search name, id, image, notes…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              style={{ marginTop: "0.75rem", width: "100%" }}
            />
          </div>

          <div className="cd-card">
            <h2>
              {entries.length} entr{entries.length === 1 ? "y" : "ies"}
              {category !== "all" ? ` in ${category}` : ""}
              {search.trim() ? ` matching “${search.trim()}”` : ""}
            </h2>
            {entries.length === 0 ? (
              <p>No entries match.</p>
            ) : (
              <table>
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Category</th>
                    <th>Image</th>
                    <th>Ports</th>
                    <th>Mem cap</th>
                    <th>Notes</th>
                  </tr>
                </thead>
                <tbody>
                  {entries.map((e) => (
                    <tr key={e.id}>
                      <td>
                        <strong>{e.name}</strong>
                        <div>
                          <code>{e.id}</code>
                        </div>
                      </td>
                      <td>{e.category}</td>
                      <td>
                        <code>{e.image}</code>
                      </td>
                      <td>{e.ports.length === 0 ? "—" : e.ports.join(", ")}</td>
                      <td>{e.mem_limit}</td>
                      <td>
                        {e.notes}
                        {e.dependencies.length > 0 && (
                          <div>
                            requires: {e.dependencies.join(", ")}
                          </div>
                        )}
                        <div>
                          <a href={e.docs} target="_blank" rel="noreferrer">
                            docs
                          </a>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </>
      )}
    </div>
  );
}
