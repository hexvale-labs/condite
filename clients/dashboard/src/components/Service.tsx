import ErrorMessage from "./ErrorMessage";
import { createSignal, onSettled } from "solid-js";
import type { HealthStatus } from "condite-sdk-wasm";
import { browserClient } from "../lib/client";
import { formatDate } from "../lib/format";
import { messages, type Locale } from "../i18n";
export default function Service(props: {
  locale: Locale;
  initial: HealthStatus | null;
  initialError?: unknown;
}) {
  const t = messages(props.locale);
  const [health, setHealth] = createSignal(() => props.initial);
  const [ready, setReady] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal<unknown>(() => props.initialError);
  const [failed, setFailed] = createSignal(!props.initial);
  onSettled(() => {
    setReady(true);
  });
  async function refresh() {
    if (busy()) return;
    setBusy(true);
    try {
      const sdk = await browserClient();
      setHealth(await sdk.health());
      setFailed(false);
      setError(null);
    } catch (cause) {
      setError(cause);
      setFailed(true);
    } finally {
      setBusy(false);
    }
  }
  return (
    <section aria-label={t.service} class="max-w-3xl">
      <div class="flex items-center justify-between gap-3">
        <h2 class="text-lg font-semibold">{t.connection}</h2>
        <button
          class="button"
          disabled={!ready() || busy()}
          onClick={() => void refresh()}
        >
          {busy() ? t.refreshing : t.refresh}
        </button>
      </div>
      <p class="feedback" role="status" aria-live="polite">
        {failed() ? t.serviceError : ""}
      </p>
      <ErrorMessage error={error()} locale={props.locale} />
      <dl class="divide-y divide-line rounded-lg border border-line bg-white px-5">
        <div class="flex flex-wrap items-center justify-between gap-3 py-5">
          <dt class="text-sm text-muted">{t.status}</dt>
          <dd
            class={[
              "badge",
              failed() || !health()?.healthy
                ? "bg-amber-50 text-amber-900"
                : "bg-emerald-50 text-emerald-800",
            ]}
          >
            {failed() ? t.offline : health()?.healthy ? t.healthy : t.unhealthy}
          </dd>
        </div>
        <div class="flex flex-wrap justify-between gap-3 py-5">
          <dt class="text-sm text-muted">{t.started}</dt>
          <dd class="text-sm tabular-nums">
            {health()
              ? formatDate(health()!.started_at, props.locale)
              : t.unknown}
          </dd>
        </div>
        <div class="flex flex-wrap justify-between gap-3 py-5">
          <dt class="text-sm text-muted">{t.runtime}</dt>
          <dd class="break-all text-sm">{health()?.runtime ?? t.unknown}</dd>
        </div>
      </dl>
      <p class="mt-4 text-xs leading-relaxed text-muted">{t.serviceHint}</p>
    </section>
  );
}
