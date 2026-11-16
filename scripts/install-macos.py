#!/usr/bin/env python3
"""Compila e instala Vectorio; --if-needed evita compilar si no hubo cambios.

La aplicación instalada abre directamente su binario: no lee el proyecto al arrancar,
así macOS no pide permiso para la carpeta Documentos en cada apertura."""

import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile

PROJECT = Path(__file__).resolve().parent.parent
BUNDLE_ID = "io.vectorio.mac"


def fingerprint():
    digest = hashlib.sha256()
    paths = [PROJECT / "Cargo.toml", PROJECT / "Cargo.lock"]
    for folder in ("crates", "scripts", "packaging/macos"):
        paths.extend(p for p in (PROJECT / folder).rglob("*")
                     if p.is_file() and "__pycache__" not in p.parts)
    paths.append(PROJECT / "assets/branding/vectorio-icon.png")
    paths.append(PROJECT / "assets/branding/vectorio-icon-macos.png")
    for path in sorted(set(paths)):
        if path.is_file():
            digest.update(str(path.relative_to(PROJECT)).encode())
            digest.update(b"\0")
            digest.update(path.read_bytes())
    return digest.hexdigest()


def run(command, env):
    subprocess.run([str(arg) for arg in command], cwd=str(PROJECT), env=env, check=True)


def install(destination, if_needed):
    destination = destination.expanduser().absolute()
    destination.parent.mkdir(parents=True, exist_ok=True)
    resource = destination / "Contents/Resources"
    stamp = resource / "build.json"
    source_hash = fingerprint()
    try:
        previous = json.loads(stamp.read_text())
    except (OSError, ValueError):
        previous = {}
    if (if_needed and previous.get("source_hash") == source_hash
            and previous.get("project") == str(PROJECT)
            and (destination / "Contents/MacOS/vectorio").is_file()
            and not (destination / "Contents/MacOS/VectorioLauncher").exists()):
        print("Vectorio ya está actualizado: {}".format(destination))
        return

    if destination.exists():
        info_path = destination / "Contents/Info.plist"
        with info_path.open("rb") as handle:
            info = plistlib.load(handle)
        if info.get("CFBundleIdentifier") != BUNDLE_ID:
            raise RuntimeError("La aplicación de destino no es esta instalación de Vectorio; no se reemplaza.")

    env = os.environ.copy()
    env["PATH"] = os.pathsep.join([
        "/opt/homebrew/opt/rustup/bin", "/opt/homebrew/bin",
        str(Path.home() / ".cargo/bin"), env.get("PATH", "/usr/bin:/bin")])
    cargo = shutil.which("cargo", path=env["PATH"])
    if not cargo:
        raise RuntimeError("No se encuentra Rust. La versión instalada se conserva.")
    icon = PROJECT / "packaging/macos/Vectorio.icns"
    if not icon.is_file():
        raise RuntimeError("Falta packaging/macos/Vectorio.icns.")

    print("Compilando Vectorio…", flush=True)
    run([cargo, "build", "--release", "--locked", "-p", "vectorio-app"], env)
    metadata = subprocess.check_output(
        [cargo, "metadata", "--no-deps", "--format-version", "1", "--locked"],
        cwd=str(PROJECT), env=env)
    binary = Path(json.loads(metadata)["target_directory"]) / "release/vectorio"
    manifest = (PROJECT / "Cargo.toml").read_text()
    version = re.search(r'(?m)^version\s*=\s*"([^"]+)"', manifest).group(1)

    # Se prepara y firma la aplicación completa antes de tocar la versión instalada.
    with tempfile.TemporaryDirectory(prefix=".Vectorio-stage-", dir=str(destination.parent)) as temporary:
        stage = Path(temporary) / "Vectorio.app"
        macos = stage / "Contents/MacOS"
        resources = stage / "Contents/Resources"
        macos.mkdir(parents=True)
        resources.mkdir(parents=True)
        shutil.copy2(binary, macos / "vectorio")
        shutil.copy2(icon, resources / "Vectorio.icns")
        shutil.copy2(PROJECT / "packaging/macos/VOLUMETRIC_NOTICES.txt", resources / "VOLUMETRIC_NOTICES.txt")
        (resources / "build.json").write_text(json.dumps({
            "project": str(PROJECT), "source_hash": source_hash, "version": version,
        }, indent=2) + "\n")
        info = {
            "CFBundleName": "Vectorio", "CFBundleDisplayName": "Vectorio",
            "CFBundleIdentifier": BUNDLE_ID, "CFBundleExecutable": "vectorio",
            "CFBundleIconFile": "Vectorio.icns", "CFBundlePackageType": "APPL",
            "CFBundleShortVersionString": version, "CFBundleVersion": version,
            "NSHighResolutionCapable": True, "NSPrincipalClass": "NSApplication",
            "NSHumanReadableCopyright": "Vectorio · MIT",
        }
        with (stage / "Contents/Info.plist").open("wb") as handle:
            plistlib.dump(info, handle)
        run(["/usr/bin/codesign", "--force", "--deep", "--sign", "-", "--identifier", BUNDLE_ID, stage], env)
        run(["/usr/bin/codesign", "--verify", "--deep", "--strict", stage], env)
        backup = Path(temporary) / "previous.app"
        if destination.exists():
            destination.rename(backup)
        try:
            stage.rename(destination)
        except BaseException:
            if backup.exists():
                backup.rename(destination)
            raise
    print("Vectorio instalado y actualizado: {}".format(destination))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--destination", type=Path, default=Path("/Applications/Vectorio.app"))
    parser.add_argument("--if-needed", action="store_true")
    arguments = parser.parse_args()
    if sys.platform != "darwin":
        parser.error("Este instalador es para macOS.")
    target = PROJECT / "target"
    target.mkdir(exist_ok=True)
    # Dos instalaciones a la vez no deben pisarse.
    with (target / "macos-install.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        install(arguments.destination, arguments.if_needed)


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print("No se ha actualizado Vectorio: {}".format(error), file=sys.stderr)
        sys.exit(1)
