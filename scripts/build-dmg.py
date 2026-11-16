#!/usr/bin/env python3
"""Genera el instalador de macOS: target/dist/Vectorio-<versión>-<arquitectura>.dmg.

El .dmg contiene Vectorio.app y un acceso a Aplicaciones para arrastrarla.
No toca la instalación de /Applications."""

import importlib.util
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("install_macos", HERE / "install-macos.py")
installer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(installer)


def main():
    if sys.platform != "darwin":
        sys.exit("El instalador .dmg solo se genera en macOS.")
    env, cargo = installer.rust_env()
    env.setdefault("MACOSX_DEPLOYMENT_TARGET", "11.0")
    print("Compilando Vectorio…", flush=True)
    installer.run([cargo, "build", "--release", "--locked", "-p", "vectorio-app"], env)
    target = installer.target_directory(cargo, env)
    binary = target / "release/vectorio"
    version = installer.project_version()
    arch = platform.machine()
    dist = target / "dist"
    dist.mkdir(parents=True, exist_ok=True)
    dmg = dist / "Vectorio-{}-{}.dmg".format(version, arch)

    with tempfile.TemporaryDirectory(prefix="vectorio-dmg-") as temporary:
        root = Path(temporary) / "Vectorio"
        root.mkdir()
        # Sin la ruta del proyecto: el .dmg se publica.
        installer.write_bundle(root / "Vectorio.app", binary, {"distribution": "dmg"}, env)
        (root / "Applications").symlink_to("/Applications")
        if dmg.exists():
            dmg.unlink()
        installer.run(["/usr/bin/hdiutil", "create", "-volname", "Vectorio", "-srcfolder", root,
                       "-fs", "HFS+", "-format", "UDZO", "-ov", dmg], env)
    print("Instalador listo: {}".format(dmg))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.CalledProcessError) as error:
        print("No se ha generado el instalador: {}".format(error), file=sys.stderr)
        sys.exit(1)
