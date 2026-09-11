import { makePlaceholder } from "./Placeholder";

export const FEATURE_IDS = [
  "plex.sessions", "plex.libraries", "plex.maintenance", "plex.butler",
  "plex.analysis", "plex.markers", "plex.backup", "plex.cleanup",
];

export default makePlaceholder({
  title: "Plex",
  featureIds: FEATURE_IDS,
  milestone: "M1 (sessions) / M3 (suite)",
});
