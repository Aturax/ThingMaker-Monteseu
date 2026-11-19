"""Fetch the exact static archives selected by sherpa-onnx-sys 2.0.1."""
import hashlib
import os
from pathlib import Path
import sys
import tempfile
import urllib.request


ARCHIVES = {
    "darwin": {
        "osx-arm64-static-lib": "d7a9b14f337958e7373c8c650f046df33d210bbaff2f33cb8322dda663ee5e1c",
        "osx-x64-static-lib": "6a3cb3c14104d3a02e7b2a6cd5621f590b14892cce7d0bc0935f7655ef518938",
    },
    "win32": {
        "win-x64-static-MT-Release-lib": "23e7b986b197eda0bbc47ff3d7d5c87afd6ef2b9077214ba26b0f1ec52ef0e66",
    },
}


def fetch(destination):
    destination.mkdir(parents=True, exist_ok=True)
    for target, expected in ARCHIVES[sys.platform].items():
        filename = f"sherpa-onnx-v2.0.1-{target}.tar.bz2"
        url = f"https://github.com/k2-fsa/sherpa-onnx/releases/download/v2.0.1/{filename}"
        with tempfile.TemporaryDirectory(dir=destination) as stage:
            archive = Path(stage) / filename
            digest = hashlib.sha256()
            with urllib.request.urlopen(url, timeout=90) as response, archive.open("wb") as output:
                while chunk := response.read(1024 * 1024):
                    digest.update(chunk)
                    output.write(chunk)
            if digest.hexdigest() != expected:
                raise ValueError(f"Unexpected checksum for {filename}")
            os.replace(archive, destination / filename)


if __name__ == "__main__":
    fetch(Path(sys.argv[1]))
