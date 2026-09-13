import datetime
import glob
import json
import os
import sys
import urllib.parse

ROOT = os.path.dirname(os.path.abspath(__file__ + "/../.."))
DIST = os.environ.get("DIST_DIR", os.path.join(ROOT, "dist-bundles"))


def find_one(patterns):
    for pattern in patterns:
        hits = sorted(glob.glob(pattern, root_dir=DIST, recursive=True))
        if hits:
            return hits[0]
    return None


def quoted(name):
    return urllib.parse.quote(name, safe="")


def main():
    tag = os.environ.get("TAG", "").strip()
    repo = os.environ.get("REPO", "").strip()
    if not tag or not repo:
        print("TAG and REPO env vars are required")
        return 1
    base = "https://github.com/" + repo + "/releases/download/" + tag + "/"
    platforms = {}
    targets = {
        "windows-x86_64": ["**/*-setup.exe"],
        "darwin-aarch64": ["**/*.app.tar.gz"],
        "linux-x86_64": ["**/*.AppImage"],
    }
    for key, patterns in targets.items():
        bundle = find_one(patterns)
        if not bundle:
            continue
        sig_path = os.path.join(DIST, bundle + ".sig")
        if not os.path.isfile(sig_path):
            continue
        with open(sig_path, encoding="utf-8") as fh:
            signature = fh.read().strip()
        if not signature:
            continue
        platforms[key] = {
            "signature": signature,
            "url": base + quoted(os.path.basename(bundle)),
        }
    if not platforms:
        print("no signed updater bundles found")
        return 1
    latest = {
        "version": tag,
        "notes": "See the release notes.",
        "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .isoformat(timespec="seconds")
        .replace("+00:00", "Z"),
        "platforms": platforms,
    }
    with open(os.path.join(DIST, "latest.json"), "w", encoding="utf-8") as fh:
        json.dump(latest, fh, ensure_ascii=False, indent=2)
        fh.write("\n")
    print("platforms: " + ", ".join(sorted(platforms)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
