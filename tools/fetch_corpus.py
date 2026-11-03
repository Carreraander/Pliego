#!/usr/bin/env python3
"""Descarga suites públicas de PDF a tests/corpus/external/ (ignorado por git).

Uso: python tools/fetch_corpus.py [--suite pdfjs|verapdf|isartor ...] [--dest DIR]

Solo usa la biblioteca estándar y `git` (clon superficial; en pdf.js solo se baja test/pdfs).
Es idempotente: si la suite ya existe, la actualiza o la omite. Licencias: son archivos de prueba de
terceros que NO se redistribuyen con Pliego; solo se usan localmente y en la CI.
"""
import argparse
import io
import subprocess
import sys
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_DEST = ROOT / "tests" / "corpus" / "external"

GIT_SUITES = {
    # nombre: (url, subcarpeta sparse o None)
    "pdfjs": ("https://github.com/mozilla/pdf.js.git", "test/pdfs"),
    "verapdf": ("https://github.com/veraPDF/veraPDF-corpus.git", None),
}
ZIP_SUITES = {
    "isartor": "https://www.pdfa.org/wp-content/uploads/2011/08/isartor-pdfa-2008-08-13.zip",
}


def run(*cmd, cwd=None):
    subprocess.run(cmd, cwd=cwd, check=True)


def fetch_git(name, dest):
    url, sparse = GIT_SUITES[name]
    target = dest / name
    if (target / ".git").is_dir():
        print(f"[{name}] ya existe, actualizando")
        run("git", "pull", "--ff-only", "--depth", "1", cwd=target)
        return
    print(f"[{name}] clonando {url}")
    cmd = ["git", "clone", "--depth", "1"]
    if sparse:
        cmd += ["--filter=blob:none", "--sparse"]
    run(*cmd, url, str(target))
    if sparse:
        run("git", "sparse-checkout", "set", sparse, cwd=target)


def fetch_zip(name, dest):
    target = dest / name
    if target.is_dir() and any(target.rglob("*.pdf")):
        print(f"[{name}] ya existe, omitido")
        return
    url = ZIP_SUITES[name]
    print(f"[{name}] descargando {url}")
    with urllib.request.urlopen(url, timeout=120) as resp:
        data = resp.read()
    target.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as zf:
        for member in zf.namelist():
            # Protección contra rutas fuera del destino.
            if (target / member).resolve().is_relative_to(target.resolve()):
                zf.extract(member, target)


def main():
    all_suites = [*GIT_SUITES, *ZIP_SUITES]
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--suite", action="append", choices=all_suites, help="suite a descargar (por defecto, todas)")
    ap.add_argument("--dest", type=Path, default=DEFAULT_DEST)
    args = ap.parse_args()
    args.dest.mkdir(parents=True, exist_ok=True)
    failed = []
    for name in args.suite or all_suites:
        try:
            (fetch_git if name in GIT_SUITES else fetch_zip)(name, args.dest)
        except Exception as e:  # una suite caída no debe impedir descargar las demás
            print(f"[{name}] ERROR: {e}", file=sys.stderr)
            failed.append(name)
    total = sum(1 for _ in args.dest.rglob("*.pdf"))
    print(f"PDF disponibles en {args.dest}: {total}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
