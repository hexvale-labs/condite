"""Package a native build without nesting another ZIP or losing addon/licenses."""
import hashlib
import pathlib
import sys
import zipfile

checkout = pathlib.Path(sys.argv[1])
output = pathlib.Path(sys.argv[2])
dist = checkout / "dist"
for required in ("patcher.js", "renderer.js", "vencordDesktopMain.js", "condite-sdk/condite-sdk.node", "condite-sdk/LICENSE", "condite-release.json"):
    if not (dist / required).is_file():
        raise SystemExit(f"Missing release output: {required}")
with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive:
    for path in sorted(dist.rglob("*")):
        if path.is_file() and path.suffix != ".zip":
            archive.write(path, path.relative_to(dist))
    archive.write(checkout / "LICENSE", "LICENSE-Vencord")
    archive.write(checkout.parent / "COPYING", "LICENSE-Condite-plugin")
    archive.write(checkout.parent.parent / "i18n/LICENSE", "LICENSE-Condite-translations")
    archive.write(checkout.parent.parent / "shared/LICENSE", "LICENSE-Condite-client-utilities")
    archive.writestr("INSTALL.txt", "Extract the entire ZIP. Keep condite-sdk beside the bundles. Native addon matches the filename OS/architecture only. For Vesktop, select this custom dist directory. See https://docs.vencord.dev/installing/ and Condite's clients/vencord-extension/README.md.\n")
output.with_suffix(".zip.sha256").write_text(f"{hashlib.sha256(output.read_bytes()).hexdigest()}  {output.name}\n", newline="\n")
