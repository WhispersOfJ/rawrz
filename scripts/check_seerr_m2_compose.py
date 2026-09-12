from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
text = (ROOT / "docker-compose.m2.yml").read_text()
assert re.search(r"^  seerr-m2:\s*$", text, re.MULTILINE)
assert "image: rawrz-seerr:m2" in text
assert '"5056:5055"' in text
assert "SEERR_CACHE_BACKEND" in text
assert "REDIS_URL" in text
assert "SEERR_CACHE_REDIS_PREFIX" in text
assert "rawrz:seerr:m2" in text
assert "config/seerr-m2:/app/config" in text
assert "depends_on" not in text
print("M2 Seerr Compose checks passed")
