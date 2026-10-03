#!/usr/bin/env python3
"""Package an already built Linux miner using an explicit file allowlist."""
from pathlib import Path
import hashlib, shutil, tarfile
root = Path(__file__).resolve().parent.parent
version = (root / "VERSION").read_text().strip()
assert version and all(part.isdigit() for part in version.split(".")) and len(version.split(".")) == 3
name = f"Eureka-Nexus-Miner-Official-{version}-Linux-x86_64"
dist = root / "dist"
package = dist / name
if package.exists():
    raise SystemExit(f"Output already exists: {package}; preserve or move it before repackaging")
package.mkdir(parents=True)
files = ["README.md", "CHANGELOG.md", "LICENSE", "SECURITY.md", "THIRD_PARTY_NOTICES.md",
         "THIRD_PARTY_RUST_LICENSES.html", "THIRD_PARTY_RUST_LICENSES.tsv", "VERSION",
         "eureka-public.json", "INSTALL_KAWPOW_ENGINE_LINUX.sh", "engines/kawpow/NOTICE.txt"]
files += [str(p.relative_to(root)) for p in (root / "LICENSES").glob("*") if p.is_file()]
for name in files:
    dest = package / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(root / name, dest)
shutil.copy2(root / "target/release/eureka-nexus-miner-official", package / "eureka-nexus-miner-official")
(package / "eureka-nexus-miner-official").chmod(0o755)
(package / "INSTALL_KAWPOW_ENGINE_LINUX.sh").chmod(0o755)
def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
rows = [f"{digest(p)}  ./{p.relative_to(package).as_posix()}" for p in sorted(package.rglob("*")) if p.is_file()]
(package / "SHA256SUMS.txt").write_text("\n".join(rows) + "\n")
archive = dist / f"{package.name}.tar.gz"
with tarfile.open(archive, "w:gz") as tar:
    tar.add(package, arcname=package.name)
checksum = f"{digest(archive)}  {archive.name}\n"
(archive.parent / (archive.name + ".sha256.txt")).write_text(checksum)
print(checksum, end="")
