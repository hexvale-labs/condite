import { test } from 'node:test';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { build } from '../.vencord/node_modules/esbuild/lib/main.js';

for (const entry of ['../src/Uptime.tsx', '../../vencord/condite.desktop/Uptime.tsx']) {
  test(`healthy refresh preserves mounted audio controller: ${entry}`, async () => {
    const state = { hooks: [], cursor: 0, effects: [], mounts: 0, updates: 0, disposals: 0 };
    globalThis.__uptimeFixture = state;
    const bundle = await build({
      entryPoints: [fileURLToPath(new URL(entry, import.meta.url))], bundle: true, write: false,
      format: 'esm', platform: 'node', jsxFactory: 'React.createElement',
      plugins: [{ name: 'discord-lifecycle', setup(builder) {
        builder.onResolve({ filter: /@webpack\/common|\.\/i18n|shared\/uptime-dom|\.css$/ }, args => ({ path: args.path, namespace: 'fixture' }));
        builder.onLoad({ filter: /.*/, namespace: 'fixture' }, ({ path }) => ({ contents:
          path.endsWith('.css') ? '' : path.endsWith('uptime-dom') ? `
            const s = globalThis.__uptimeFixture;
            export function mountUptime() { s.mounts++; const dispose = () => s.disposals++; dispose.update = () => s.updates++; return dispose; }
          ` : path.endsWith('i18n') ? 'export const useTranslation = () => ({locale:"en", t:{}});' : `
            const s = globalThis.__uptimeFixture;
            export const React = {createElement(tag, props) { if (props.ref) props.ref.current = {}; return {}; }};
            export function useRef(initial) { const i = s.cursor++; return s.hooks[i] ??= {current: initial}; }
            export function useEffect(fn, deps) {
              const i = s.cursor++; const old = s.hooks[i];
              if (!old || deps.some((value, index) => value !== old.deps[index])) {
                s.effects.push(() => { old?.cleanup?.(); s.hooks[i] = { deps, cleanup: fn() }; });
              }
            }
          ` }));
      } }]
    });
    const { Uptime } = await import(`data:text/javascript;base64,${Buffer.from(bundle.outputFiles[0].text).toString('base64')}`);
    const render = health => { state.cursor = 0; Uptime({health}); state.effects.splice(0).forEach(effect => effect()); };
    render({ healthy: true, runtime: 'PT1S', started_at: '' });
    render({ healthy: true, runtime: 'PT31S', started_at: '' });
    assert.equal(state.mounts, 1);
    assert.equal(state.disposals, 0);
    assert.ok(state.updates >= 1);
    for (const hook of state.hooks) hook?.cleanup?.();
    assert.equal(state.disposals, 1);
    delete globalThis.__uptimeFixture;
  });
}
