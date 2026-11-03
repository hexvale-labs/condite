import { createHash } from 'node:crypto';
import { execFileSync, spawnSync } from 'node:child_process';
import { appendFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { root, tagMetadata } from './version.mjs';
export const imageComponents = component => component === 'umbrella' ? ['core', 'dashboard'] : ['core', 'dashboard'].filter(c => c === component);
export function publication(existing, fingerprint) {
  if (!existing) return { build: true };
  if (existing.fingerprint !== fingerprint) throw new Error('Published component inputs changed; bump its version before releasing.');
  return { build: false, digest: existing.digest };
}
export function imageFingerprint(info) {
  const platforms = Object.entries(info).filter(([name]) => name === 'linux/amd64' || /^linux\/arm64(?:\/v8)?$/.test(name));
  if (!platforms.some(([name]) => name === 'linux/amd64') || !platforms.some(([name]) => name.startsWith('linux/arm64'))) throw new Error('Published image is missing required Linux platforms');
  const labels = platforms.map(([, image]) => image.config?.Labels?.['io.condite.inputs']);
  if (!labels[0] || !labels.every(value => value === labels[0])) throw new Error('Published image has inconsistent input fingerprints');
  return labels[0];
}
export function fingerprint(component, ref = 'HEAD', directory = root, dashboardOrigin = process.env.DASHBOARD_ORIGIN || 'http://localhost:4321') {
  const common = ['Cargo.toml', 'Cargo.lock', '.cargo/', 'docker/', '.dockerignore', 'scripts/release/', '.github/workflows/images.yml'];
  const inputs = component === 'core' ? ['crates/condite-core/', 'crates/condite-ctl/', 'crates/condite-shared/'] : ['crates/', 'clients/dashboard/', 'clients/i18n/', 'clients/shared/', 'scripts/sdk/', 'package.json', 'bun.lock'];
  const entries = execFileSync('git', ['ls-tree', '-r', ref], { cwd: directory, encoding: 'utf8' }).split('\n').filter(line => {
    const path = line.split('\t')[1];
    if (!path) return false;
    if (component === 'dashboard' && (path.startsWith('crates/condite-core/') || path.startsWith('crates/condite-ctl/'))) return false;
    return [...common, ...inputs].some(p => p.endsWith('/') ? path.startsWith(p) : path === p);
  });
  return createHash('sha256').update(entries.join('\n')).update(component === 'dashboard' ? new URL(dashboardOrigin).origin : '').digest('hex');
}
function docker(args, options = {}) { return execFileSync('docker', args, { encoding: 'utf8', ...options }).trim(); }
export function inspectImage(image) {
  const probe = spawnSync('docker', ['buildx', 'imagetools', 'inspect', image], { encoding: 'utf8' });
  if (probe.status !== 0) {
    // Fail closed on auth/network failures, not just on existing images.
    if (/manifest unknown|not found|no such manifest/i.test(probe.stderr)) return null;
    throw new Error(probe.stderr || 'Cannot query registry');
  }
  const digest = /^Digest:\s+(sha256:[a-f0-9]+)/m.exec(probe.stdout)?.[1];
  if (!digest) throw new Error('Missing registry digest');
  const info = JSON.parse(docker(['buildx', 'imagetools', 'inspect', image, '--format', '{{json .Image}}']));
  return { digest, fingerprint: imageFingerprint(info) };
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [action, tag, component] = process.argv.slice(2);
    const metadata = tagMetadata(tag);
    if (action === 'matrix') console.log(JSON.stringify({ include: imageComponents(metadata.component).flatMap(component => [
      { component, arch: 'amd64', runner: 'ubuntu-24.04' }, { component, arch: 'arm64', runner: 'ubuntu-24.04-arm' }
    ]) }));
    else if (action === 'prepare') {
      if (!imageComponents(metadata.component).includes(component)) throw new Error('Unexpected image component');
      const image = `ghcr.io/hexvale-labs/condite/${component}`;
      const version = metadata[component];
      // Docker tags cannot contain SemVer '+'. Encoding preserves identity.
      const versionTag = version.replace('+', '_');
      const hash = fingerprint(component);
      const decision = publication(inspectImage(`${image}:${versionTag}`), hash);
      const values = { image, version, versionTag, fingerprint: hash, build: String(decision.build), digest: decision.digest || '' };
      if (process.env.GITHUB_OUTPUT) for (const [k,v] of Object.entries(values)) appendFileSync(process.env.GITHUB_OUTPUT, `${k}=${v}\n`);
      console.log(JSON.stringify(values));
    } else throw new Error('Use matrix TAG | prepare TAG COMPONENT');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
