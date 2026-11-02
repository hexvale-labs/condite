import { execFileSync } from 'node:child_process';
import { mkdirSync, rmSync, writeFileSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseTag, root, tagMetadata } from './version.mjs';
export const releaseTargets = [
  { os: 'linux', arch: 'x64', runner: 'ubuntu-24.04', rustHost: 'x86_64-unknown-linux-gnu' },
  { os: 'linux', arch: 'arm64', runner: 'ubuntu-24.04-arm', rustHost: 'aarch64-unknown-linux-gnu' },
  { os: 'win32', arch: 'x64', runner: 'windows-2025', rustHost: 'x86_64-pc-windows-msvc' },
  { os: 'win32', arch: 'arm64', runner: 'windows-11-arm', rustHost: 'aarch64-pc-windows-msvc' },
  { os: 'darwin', arch: 'x64', runner: 'macos-15-intel', rustHost: 'x86_64-apple-darwin' },
  { os: 'darwin', arch: 'arm64', runner: 'macos-15', rustHost: 'aarch64-apple-darwin' }
];
export function releaseAllowed(action, tag) {
  try { return action === 'published' && ['umbrella', 'vencord'].includes(parseTag(tag).component); } catch { return false; }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [action, tag] = process.argv.slice(2);
    if (!releaseAllowed('published', tag)) throw new Error('Not a Vencord/umbrella SemVer release');
    const metadata = tagMetadata(tag);
    if (action === 'matrix') console.log(JSON.stringify({ include: releaseTargets }));
    else if (action === 'package') {
      const cwd = resolve(root, 'clients/vencord-extension');
      const checkout = resolve(cwd, '.vencord');
      const run = (command, args, directory = root) => execFileSync(command, args, { cwd: directory, stdio: 'inherit', shell: process.platform === 'win32' });
      mkdirSync(resolve(root, 'release-assets'), { recursive: true });
      for (const variant of ['desktop', 'standalone']) {
        rmSync(resolve(checkout, 'dist'), { recursive: true, force: true });
        run('node', ['tooling.mjs', variant === 'standalone' ? 'build:standalone' : 'build'], cwd);
        writeFileSync(resolve(checkout, 'dist/condite-release.json'), JSON.stringify({
          ...metadata, variant, platform: process.platform, arch: process.arch,
          revision: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
          upstream: JSON.parse(readFileSync(resolve(cwd, 'vencord.json'), 'utf8'))
        }, null, 2) + '\n');
        const name = `condite-vencord-${metadata.vencord}-${variant}-${process.platform}-${process.arch}.zip`;
        run('python', [resolve(root, 'scripts/release/zip-vencord.py'), checkout, resolve(root, 'release-assets', name)]);
      }
    } else throw new Error('Use matrix TAG | package TAG');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
