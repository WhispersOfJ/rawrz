#!/usr/bin/env python3
"""Static guard: no data-sized env-var transports into embedded python.

Bulk payloads (series maps, history, full-collection pulls) must be piped
to `python3` renderers via stdin, never through environment variables: a
single env var is capped at 128 KB (MAX_ARG_STRLEN) on Linux, so a payload
that scales with library size can fail `execve` with E2BIG silently.

Scans services/bash-functions/functions/*.sh for env-prefix assignments
directly before a `python3` invocation and fails on any variable not in the
scalar whitelist (URLs, keys, IDs, limits, app names, state). Add new
scalar env vars to SCALAR_ENV_VARS after review; anything data-sized must
go via stdin instead. See docs/stack/services/bash-functions.md.
"""

import re
import sys
from pathlib import Path

SCALAR_ENV_VARS = {
    "APP",       # radarr|sonarr
    "ENDPOINT",  # api path segment
    "ID",        # single record id
    "KEY",       # api key
    "LIMIT",     # integer limit argument
    "PLEX_URL",  # plex host
    "STATE",     # on|off
    "TOKEN",     # plex token
    "URL",       # host base url
}

FUNC_DIR = (
    Path(__file__).resolve().parent.parent
    / "services" / "bash-functions" / "functions"
)

# A simple `VAR=value` token immediately before python3. The value must be a
# closed quote or a bare word (no $(...) or unbalanced quotes), so outer
# assignments like `count="$(STATE=... python3 ...)"` don't match.
TOKEN_RE = re.compile(
    r"^([A-Za-z_][A-Za-z0-9_]*)=(?:\"[^\"]*\"|'[^']*'|[^\s\"'()]*)$"
)


def env_prefixes(line: str) -> list[str]:
    if "python3" not in line:
        return []
    head = re.split(r"\bpython3\b", line, maxsplit=1)[0]
    return [
        match.group(1)
        for tok in head.split()
        if (match := TOKEN_RE.match(tok))
    ]


def main() -> int:
    repo_root = FUNC_DIR.parent.parent.parent
    bad: list[str] = []
    for path in sorted(FUNC_DIR.glob("*.sh")):
        for lineno, line in enumerate(
            path.read_text(encoding="utf-8").splitlines(), 1
        ):
            for var in env_prefixes(line):
                if var not in SCALAR_ENV_VARS:
                    rel = path.relative_to(repo_root)
                    bad.append(f"{rel}:{lineno}: {var}=... python3")

    if bad:
        print(
            "Data-sized env-var transports into python "
            "(must be piped via stdin):",
            file=sys.stderr,
        )
        for entry in bad:
            print(f"  {entry}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
