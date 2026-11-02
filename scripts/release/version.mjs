import { readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
export const root = fileURLToPath(new URL('../../', import.meta.url));
const numeric = '(0|[1-9][0-9]*)';
const identifier = '(?:0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)';
const semver = new RegExp(`^${numeric}\\.${numeric}\\.${numeric}(?:-${identifier}(?:\\.${identifier})*)?(?:\\+[0-9A-Za-z-]+(?:\\.[0-9A-Za-z-]+)*)?$`);
export const validVersion = value => typeof value === 'string' && semver.test(value);
export function parseTag(tag) {
  const match = /^(?:(core|sdk|dashboard|vencord)\/)?v(.+)$/.exec(tag);
  if (!match || !validVersion(match[2])) throw new Error(`Invalid release tag: ${tag}`);
  return { component: match[1] || 'umbrella', version: match[2] };
}
export function versions(directory = root) { return JSON.parse(readFileSync(resolve(directory, 'versions.json'), 'utf8')); }
const crates = { core: ['condite-core', 'condite-ctl'], sdk: ['condite-sdk', 'condite-sdk-node', 'condite-sdk-wasm', 'condite-shared', 'condite-metadata'] };
const packages = { sdk: ['crates/condite-sdk-node/package.json', 'crates/condite-sdk-wasm/package.json'], dashboard: ['clients/dashboard/package.json'], vencord: ['clients/vencord-extension/package.json', 'clients/vencord/package.json'] };
function expectedFiles(v, directory = root) {
  const files = new Map();
  for (const [component, names] of Object.entries(crates)) for (const name of names) {
    const file = `crates/${name}/Cargo.toml`;
    files.set(file, readFileSync(resolve(directory, file), 'utf8').replace(/^(version = ")[^"]+/m, `$1${v[component]}`));
  }
  for (const [component, names] of Object.entries(packages)) for (const file of names) {
    const data = JSON.parse(readFileSync(resolve(directory, file), 'utf8'));
    data.version = v[component];
    files.set(file, JSON.stringify(data, null, 2) + '\n');
  }
  const file = 'Cargo.toml';
  files.set(file, readFileSync(resolve(directory, file), 'utf8').replace(/^(condite-[\w-]+ = \{ version = ")[^"]+/gm, `$1${v.sdk}`));
  return files;
}
export function checkVersions(directory = root) {
  const v = versions(directory);
  for (const component of ['umbrella', 'core', 'sdk', 'dashboard', 'vencord']) if (!validVersion(v[component])) throw new Error(`Invalid ${component} version`);
  for (const [file, expected] of expectedFiles(v, directory)) if (readFileSync(resolve(directory, file), 'utf8').replaceAll('\r\n', '\n') !== expected.replaceAll('\r\n', '\n')) throw new Error(`Version drift: ${file}. Run just version COMPONENT VERSION.`);
  for (const file of ['Cargo.lock', 'examples/sdk-consumer/Cargo.lock']) {
    const content = readFileSync(resolve(directory, file), 'utf8').replaceAll('\r\n', '\n');
    for (const [component, names] of Object.entries(crates)) for (const name of names) {
      const entry = content.split('[[package]]').find(part => part.includes(`name = "${name}"\n`));
      if (entry && !entry.includes(`version = "${v[component]}"\n`)) throw new Error(`Version drift: ${file}: ${name}`);
    }
  }
  return v;
}
export function tagMetadata(tag) {
  const info = parseTag(tag), v = checkVersions();
  if (v[info.component] !== info.version) throw new Error(`Tag ${tag} does not match versions.json`);
  return { ...v, ...info, isUmbrella: info.component === 'umbrella' };
}
function run(command, args) {
  const p = spawnSync(command, args, { cwd: root, stdio: 'inherit' });
  if (p.status !== 0) throw new Error(`${command} failed`);
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [action, component, version] = process.argv.slice(2);
    if (action === 'check') checkVersions();
    else if (action === 'tag') console.log(JSON.stringify(tagMetadata(component)));
    else if (action === 'set') {
      const v = versions();
      if (!Object.hasOwn(v, component) || !validVersion(version)) throw new Error('Use set core|sdk|dashboard|vencord|umbrella X.Y.Z');
      v[component] = version;
      writeFileSync(resolve(root, 'versions.json'), JSON.stringify(v, null, 2) + '\n');
      for (const [file, content] of expectedFiles(v)) writeFileSync(resolve(root, file), content);
      run('cargo', ['update', '--workspace']);
      run('cargo', ['update', '--manifest-path', 'examples/sdk-consumer/Cargo.toml', '--workspace']);
      run('bun', ['install', '--lockfile-only']);
      checkVersions();
    } else throw new Error('Use check | tag TAG | set COMPONENT VERSION');
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
