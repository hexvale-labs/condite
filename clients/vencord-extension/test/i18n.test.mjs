import assert from 'node:assert/strict';
import { test } from 'node:test';
import { build } from '../.vencord/node_modules/esbuild/lib/main.js';

const result = await build({
    entryPoints: [new URL('../.vencord/src/userplugins/condite/i18n.ts', import.meta.url).pathname],
    bundle: true, write: false, format: 'esm', platform: 'browser',
    plugins: [{ name: 'discord-language-fixture', setup(build) {
        build.onResolve({ filter: /^@webpack\/common$/ }, () => ({ path: 'common', namespace: 'fixture' }));
        build.onLoad({ filter: /.*/, namespace: 'fixture' }, () => ({ contents: `
            export const LocaleStore = globalThis.__conditeLocale;
            export function useStateFromStores(stores, selector) {
                if (stores[0] !== LocaleStore) throw new Error('Missing locale subscription');
                return selector();
            }
        ` }));
    } }]
});
globalThis.__conditeLocale = { locale: 'pl' };
const i18n = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString('base64')}`);

test('Condite reads Discord language and reselects translated text and direction', () => {
    assert.equal(i18n.t('files'), 'Pliki');
    globalThis.__conditeLocale.locale = 'ar';
    assert.equal(i18n.useTranslation().dir, 'rtl');
    assert.equal(i18n.useTranslation().t.files, 'الملفات');
    globalThis.__conditeLocale.locale = 'pt-PT';
    assert.equal(i18n.useTranslation().locale, 'pt-BR');
    globalThis.__conditeLocale.locale = 'unsupported';
    assert.equal(i18n.t('files'), 'Files');
});
