interface PlaceholderProps {
  title: string;
  featureIds: string[];
  milestone: string;
}

/** M0 placeholder for area pages. Each page declares the FEATURE_IDS it serves —
 * scripts/check_api_contract.py scrapes this export for coverage (§D.4). */
export function makePlaceholder({ title, featureIds, milestone }: PlaceholderProps) {
  function Page() {
    return (
      <div>
        <h1>{title}</h1>
        <div className="cd-card">
          <p>
            Arrives in <strong>{milestone}</strong>. Contract already pinned:
            feature IDs <code>{featureIds.join(", ")}</code>.
          </p>
        </div>
      </div>
    );
  }
  (Page as unknown as { FEATURE_IDS: string[] }).FEATURE_IDS = featureIds;
  return Page;
}

export const FEATURE_IDS_PLACEHOLDER = true;
