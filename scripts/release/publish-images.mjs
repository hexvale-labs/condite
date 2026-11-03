import { readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { tagMetadata } from './version.mjs';
import { imageComponents, fingerprint, inspectImage, publication } from './images.mjs';
const metadata = tagMetadata(process.argv[2]);
for (const component of imageComponents(metadata.component)) {
  const image = `ghcr.io/hexvale-labs/condite/${component}`;
  const version = metadata[component].replace('+', '_');
  const existing = inspectImage(`${image}:${version}`);
  const decision = publication(existing, fingerprint(component));
  const digests = [...new Set(['amd64', 'arm64'].map(arch => readFileSync(`digests/${component}-${arch}`, 'utf8').trim()))];
  if (!digests.every(d => /^sha256:[a-f0-9]{64}$/.test(d))) throw new Error('Invalid image digest');
  const run = args => execFileSync('docker', ['buildx', 'imagetools', 'create', ...args], { stdio: 'inherit' });
  if (decision.build) run(['--tag', `${image}:${version}`, ...digests.map(d => `${image}@${d}`)]);
  if (metadata.isUmbrella) run(['--tag', `${image}:release-${metadata.version.replace('+', '_')}`, `${image}:${version}`]);
}
