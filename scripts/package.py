#!/usr/bin/env python3
"""Package a built widget as the NAR Deck's Install Widget accepts.

Follows upstream's docs/uploading-wasm-widgets.md. Needs python3 with jsonschema only:
the NAR is serialised here rather than by `nix-store --dump`.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import struct
import subprocess
import tempfile
import tomllib

import jsonschema

ROOT = Path(__file__).resolve().parents[1]
NAME = re.compile(r"[a-z0-9]+(?:-[a-z0-9]+)*")
LFS_POINTER = b"version https://git-lfs.github.com/spec/v1"


def nar_string(out, data):
    out.write(struct.pack("<Q", len(data)))
    out.write(data)
    out.write(b"\0" * (-len(data) % 8))


def nar_node(out, path):
    nar_string(out, b"(")
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode):
        for token in (b"type", b"symlink", b"target", os.fsencode(os.readlink(path))):
            nar_string(out, token)
    elif stat.S_ISDIR(mode):
        nar_string(out, b"type")
        nar_string(out, b"directory")
        for name in sorted(os.fsencode(entry) for entry in os.listdir(path)):
            for token in (b"entry", b"(", b"name", name, b"node"):
                nar_string(out, token)
            nar_node(out, path / os.fsdecode(name))
            nar_string(out, b")")
    else:
        nar_string(out, b"type")
        nar_string(out, b"regular")
        if mode & stat.S_IXUSR:
            nar_string(out, b"executable")
            nar_string(out, b"")
        nar_string(out, b"contents")
        nar_string(out, path.read_bytes())
    nar_string(out, b")")


def write_nar(out, root):
    nar_string(out, b"nix-archive-1")
    nar_node(out, root)


def sdk_version(upstream):
    source = (upstream / "bmc-wasm-runtime/protocol/src/version.rs").read_text()
    found = re.findall(r"^pub const SDK_VERSION: \(u16, u16, u16\) = \((\d+), (\d+), (\d+)\);$",
                       source, re.MULTILINE)
    if len(found) != 1:
        raise SystemExit("Expected exactly one SDK_VERSION declaration in the pinned SDK")
    return found[0]


def sdk_revision():
    lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
    sources = {package.get("source", "") for package in lock["package"] if package["name"] == "bmc-wasm-sdk"}
    if len(sources) != 1 or "#" not in next(iter(sources)):
        raise SystemExit("Cargo.lock must hold exactly one bmc-wasm-sdk, from Git")
    return next(iter(sources)).rsplit("#", 1)[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--wasm", type=Path, required=True, help="the widget module cargo built")
    parser.add_argument("--artifact-root", type=Path, required=True, help="cargo's deps directory beside it")
    parser.add_argument("--extractor", type=Path, required=True, help="the bmc-wasm-assets binary")
    parser.add_argument("--upstream", type=Path, required=True, help="the pinned bmc-main checkout")
    parser.add_argument("--upstream-revision", required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()

    name = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["name"]
    maintainer = json.loads((ROOT / "packaging.json").read_text())["maintainer"]
    if not isinstance(maintainer, str) or not NAME.fullmatch(maintainer):
        raise SystemExit("Maintainer must contain lowercase letters, digits and dashes")
    manifest = json.loads((ROOT / "manifest.json").read_text())
    schema = json.loads((args.upstream / "bmc-widget-manifest/manifest.schema.json").read_text())
    jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker()).validate(manifest)
    if manifest["binary"] != f"bin/{name}":
        raise SystemExit("Manifest binary must match the crate name")
    if not manifest["name"].endswith(f" v{manifest['version']}"):
        raise SystemExit("Widget display name must end with ' v' and its release version")
    if args.wasm.read_bytes()[:8] != b"\x00asm\x01\x00\x00\x00":
        raise SystemExit(f"{args.wasm} is not a WebAssembly module")
    sdk = sdk_version(args.upstream)
    # The launcher, schema and extractor come from --upstream, the wasm from Cargo.lock's SDK.
    if sdk_revision() != args.upstream_revision:
        raise SystemExit(f"Cargo.lock's bmc-wasm-sdk is at {sdk_revision()}, "
                         f"but the packaging files are from {args.upstream_revision}")

    with tempfile.TemporaryDirectory() as staging:
        package = Path(staging) / "package"
        widget = package / "lib/bmc-widgets" / maintainer / name
        (widget / "bin").mkdir(parents=True)
        (widget / "lib/wasm").mkdir(parents=True)
        shutil.copyfile(ROOT / "manifest.json", widget / "manifest.json")
        packaged_wasm = widget / f"lib/wasm/{name}.wasm"
        subprocess.run([args.extractor, "extract", "--input", args.wasm, "--wasm-output", packaged_wasm,
                        "--asset-root", widget / "lib/assets", "--artifact-root", args.artifact_root], check=True)
        subprocess.run([args.extractor, "verify-stripped", "--input", packaged_wasm], check=True)
        for directory in ("assets", "lib/assets"):
            if (ROOT / directory).exists():
                shutil.copytree(ROOT / directory, widget / directory, dirs_exist_ok=True)
        template = (args.upstream / "nix/wasm-widget-wrapper.sh").read_text()
        launcher = widget / "bin" / name
        launcher.write_text(template.replace("@sdkMajor@", sdk[0]).replace("@name@", name))
        for path in package.rglob("*"):
            path.chmod(0o755 if path.is_dir() or path == launcher else 0o644)
            # A checkout without LFS content would otherwise ship the pointer text in place of the file.
            if path.is_file() and LFS_POINTER in path.read_bytes():
                raise SystemExit(f"{path.relative_to(package)} carries a Git LFS pointer, not its content")

        args.out.mkdir(parents=True, exist_ok=True)
        artifact = args.out / f"{name}-{manifest['version']}.nar"
        with artifact.open("wb") as stream:
            write_nar(stream, package)

    metadata = {
        "widget": name,
        "maintainer": maintainer,
        "package": f"widget-{maintainer}_{name}",
        "version": manifest["version"],
        "upstream_revision": args.upstream_revision,
        "sdk_version": ".".join(sdk),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "cargo_lock_sha256": hashlib.sha256((ROOT / "Cargo.lock").read_bytes()).hexdigest(),
        "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
        "bytes": artifact.stat().st_size,
    }
    artifact.with_suffix(".nar.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata, indent=2))


if __name__ == "__main__":
    main()
