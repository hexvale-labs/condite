import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { build } from "../.vencord/node_modules/esbuild/lib/main.js";

const english = JSON.parse(readFileSync(new URL("../../i18n/en.json", import.meta.url), "utf8"));
const hooks = [];
let cursor = 0;
globalThis.__conditeErrorReact = {
    createElement: (type, props, ...children) => ({ type, props: { ...props, children } }),
    useState: initial => {
        const index = cursor++;
        if (!(index in hooks)) hooks[index] = initial;
        return [hooks[index], value => { hooks[index] = value; }];
    },
    useRef: initial => {
        const index = cursor++;
        return hooks[index] ??= { current: initial };
    }
};
const result = await build({
    entryPoints: [new URL('../.vencord/src/userplugins/condite/errors.tsx', import.meta.url).pathname],
    bundle: true, write: false, format: 'esm', platform: 'browser',
    plugins: [{ name: 'error-ui-fixture', setup(build) {
        build.onResolve({ filter: /^@webpack\/common$/ }, () => ({ path: 'common', namespace: 'fixture' }));
        build.onResolve({ filter: /^\.\/i18n$/ }, () => ({ path: 'i18n', namespace: 'fixture' }));
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, args => ({ contents: args.path === 'common'
            ? 'export const React = globalThis.__conditeErrorReact; export const Modal = "modal"; export function openModal() {}'
            : `export function useTranslation() { return { t: ${JSON.stringify(english)} }; }`
        }));
    } }]
});

const ui = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`);

test('Vencord error disclosure formats structured errors and retains raw details', () => {
    const tree = ui.ErrorMessage({ error: { status: 400, body: '{"title":"Sharing unavailable","detail":"Configure storage","debug":"canary"}' } });
    const [message, details] = tree.props.children;
    assert.equal(message.props.children[0], 'Sharing unavailable: Configure storage');
    assert.equal(details.type, 'details');
    assert.equal(details.props.open, undefined);
    assert.equal(details.props.children[0].props.children[0], 'Raw error');
    assert.match(details.props.children[1].props.children[0], /canary/);
});

test('confirmation failures keep the dialog open with raw details and allow retry', async () => {
    hooks.length = 0;
    let closed = false;
    const failure = new Error('HTTP 500: unavailable');
    let fail = true;
    const props = { confirmText: 'Delete', cancelText: 'Cancel', onClose: () => { closed = true; }, onConfirm: async () => { if (fail) throw failure; } };
    const render = () => { cursor = 0; return ui.ErrorConfirmModal(props); };
    await render().props.actions[0].onClick();
    assert.equal(closed, false);
    const tree = render();
    assert.equal(tree.props.children[1].props.error, failure);
    assert.equal(tree.props.actions[0].disabled, false);
    fail = false;
    await tree.props.actions[0].onClick();
    assert.equal(closed, true);
});
