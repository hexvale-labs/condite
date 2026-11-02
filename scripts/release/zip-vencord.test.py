import pathlib
import subprocess
import tempfile
import unittest
import zipfile

class PackageTest(unittest.TestCase):
    def test_balanced_compression_addon_and_licenses(self):
        with tempfile.TemporaryDirectory() as directory:
            base = pathlib.Path(directory)
            checkout = base / '.vencord'
            dist = checkout / 'dist'
            for name in ('patcher.js', 'renderer.js', 'vencordDesktopMain.js', 'condite-sdk/condite-sdk.node', 'condite-sdk/LICENSE', 'condite-release.json'):
                path = dist / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('fixture' * 100)
            (checkout / 'LICENSE').write_text('GPL upstream')
            (base / 'COPYING').write_text('GPL integration')
            output = base / 'build.zip'
            subprocess.run(['python3', 'scripts/release/zip-vencord.py', str(checkout), str(output)], check=True)
            with zipfile.ZipFile(output) as archive:
                self.assertIn('condite-sdk/condite-sdk.node', archive.namelist())
                self.assertIn('LICENSE-Condite-plugin', archive.namelist())
                self.assertTrue(all(item.compress_type == zipfile.ZIP_DEFLATED for item in archive.infolist()))
            self.assertTrue(output.with_suffix('.zip.sha256').exists())

if __name__ == '__main__': unittest.main()
