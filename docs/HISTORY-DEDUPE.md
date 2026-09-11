# Spec and history deduplication

M0 intentionally keeps one canonical RPG specification in the RAWRZ tree.

| Copy | Size observed | Disposition |
|---|---:|---|
| `docs/rpg/movie-rpg-spec.md` imported from `WhispersOfJ/movie-rpg` | 175,503 bytes | **Canonical** copy retained |
| `~/Cave/movie-rpg-spec.md` | 160,427 bytes | Stale Bear Cave copy; not imported |
| `~/movie-rpg-spec.md` | 25,750 bytes | Older standalone copy; not imported |

The stale files were outside the RAWRZ worktree and were never treated as source of
truth. Changes to RPG design should update `docs/rpg/movie-rpg-spec.md` together with
the relevant RAWRZ master-spec decision.

The former partial stack-management Rust sources under `~/Cave/backend/src/` were also
not committed in the Bear Cave history. Their inventory and disposition are recorded in
[`docs/stack/legacy-backend.md`](stack/legacy-backend.md); the implemented Deck control
plane is the successor.
