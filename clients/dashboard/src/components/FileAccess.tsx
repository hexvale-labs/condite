import { createMemo, createSignal, For, Show } from "solid-js";
import type { Share, Upload } from "condite-sdk-wasm";
import { browserClient } from "../lib/client";
import { formatDate, safeDownloadUrl } from "../lib/format";
import { defaultExpiry, resolveExpiry, type ExpiryChoice } from "../lib/expiry";
import { messages, type Locale } from "../i18n";
import ExpiryPicker from "./ExpiryPicker";

export type AccessMode = "share" | "links" | "expiry";
export type FileAccessHandle = {
  open: (row: Upload, mode: AccessMode) => void;
};
type Props = {
  locale: Locale;
  onReady: (handle: FileAccessHandle) => void;
  onChanged: () => Promise<void>;
  onDeleted: () => void;
};

export default function FileAccess(props: Props) {
  const t = messages(props.locale);
  const [row, setRow] = createSignal<Upload | null>(null);
  const [mode, setMode] = createSignal<AccessMode>("share");
  const [choice, setChoice] = createSignal<ExpiryChoice>(defaultExpiry());
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");
  const [notice, setNotice] = createSignal("");
  const [shares, setShares] = createSignal<Share[]>([]);
  const [next, setNext] = createSignal<string | null>(null);
  const [after, setAfter] = createSignal<string | undefined>();
  const [created, setCreated] = createSignal<Share | null>(null);
  const [confirmDeletion, setConfirmDeletion] = createSignal(false);
  let dialog!: HTMLDialogElement;
  let session = 0;
  const deadline = createMemo(() => {
    try {
      return resolveExpiry(
        choice(),
        mode() === "expiry" ? row()?.expires_at : null,
      );
    } catch {
      return "invalid";
    }
  });
  const deleting = () =>
    typeof deadline() === "string" &&
    deadline() !== "invalid" &&
    Date.parse(deadline()!) <= Date.now();
  const displayDate = (value: string | null) =>
    value ? `${formatDate(value, props.locale)} UTC` : t.never;
  const shareStatus = (share: Share) =>
    ({
      Creating: t.creating,
      Active: t.active,
      Revoking: t.revoking,
      Revoked: t.revoked,
      Expired: t.expired,
    })[share.status];

  async function loadShares(cursor?: string) {
    const selected = row();
    if (!selected) return;
    const current = session;
    setBusy(true);
    setError("");
    try {
      const page = await (
        await browserClient()
      ).listShares(selected.id, { limit: 20, after: cursor });
      if (current !== session) return;
      setShares(page.shares);
      setNext(page.next_after);
      setAfter(cursor);
      return page;
    } catch {
      if (current === session) setError(t.error);
    } finally {
      if (current === session) setBusy(false);
    }
  }
  props.onReady({
    open(selected, selectedMode) {
      session++;
      setRow(selected);
      setMode(selectedMode);
      setError("");
      setNotice("");
      setCreated(null);
      setShares([]);
      setNext(null);
      setAfter(undefined);
      setBusy(false);
      setConfirmDeletion(false);
      setChoice({
        ...defaultExpiry(),
        mode:
          selectedMode === "share"
            ? "upload"
            : selected.expires_at
              ? "date"
              : "never",
        date: selected.expires_at?.slice(0, 16) ?? "",
      });
      dialog.showModal();
      if (selectedMode === "links") queueMicrotask(() => void loadShares());
    },
  });
  async function copy(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      setNotice(t.copied);
    } catch {
      setNotice(t.copyFailed);
    }
  }
  async function save() {
    const selected = row();
    if (!selected || busy()) return;
    const expires = deadline();
    if (expires === "invalid" || (deleting() && mode() === "share")) {
      setError(t.invalidExpiry);
      return;
    }
    if (deleting() && !confirmDeletion()) {
      setError(t.deletionWarning);
      return;
    }
    const current = session;
    const removesFile = deleting() && mode() === "expiry";
    setBusy(true);
    setError("");
    setNotice("");
    try {
      const sdk = await browserClient();
      if (mode() === "share") {
        const share = await sdk.createShare(selected.id, {
          expires_at: expires ?? null,
          follows_upload: choice().mode === "upload",
        });
        if (!safeDownloadUrl(share.url)) throw new Error("Invalid share URL");
        if (current === session) {
          setCreated(share);
          setNotice(t.shareCreated);
        }
      } else if (deleting()) {
        await sdk.delete(selected.id);
        if (current === session) dialog.close();
      } else {
        const updated = await sdk.updateRetention(selected.id, {
          expires_at: expires ?? null,
        });
        if (current === session) {
          setRow(updated);
          setNotice(t.expirySaved);
        }
      }
      await props.onChanged();
      if (removesFile) props.onDeleted();
    } catch {
      if (current === session) setError(t.error);
    } finally {
      if (current === session) setBusy(false);
    }
  }
  async function revoke(share: Share) {
    const selected = row();
    if (!selected || busy()) return;
    const current = session;
    setBusy(true);
    setError("");
    setNotice("");
    try {
      await (await browserClient()).revokeShare(selected.id, share.id);
      if (current !== session) return;
      const page = await loadShares(after());
      if (current !== session) return;
      const updated = page?.shares.find((item) => item.id === share.id);
      setNotice(
        updated &&
          (updated.status === "Revoked" || updated.status === "Expired")
          ? shareStatus(updated)
          : t.cleanupPending,
      );
      await props.onChanged();
    } catch {
      if (current === session) setError(t.error);
    } finally {
      if (current === session) setBusy(false);
    }
  }
  return (
    <dialog
      ref={(el) => {
        dialog = el;
      }}
      class="file-dialog"
      aria-labelledby="access-title"
      onClose={() => {
        session++;
        setRow(null);
      }}
    >
      <header class="flex items-start justify-between gap-4">
        <div>
          <h2 id="access-title" class="text-xl font-semibold tracking-tight">
            {mode() === "share"
              ? t.share
              : mode() === "links"
                ? t.manageLinks
                : t.retention}
          </h2>
          <p class="mt-1 break-all text-sm text-muted">{row()?.filename}</p>
        </div>
        <button
          class="button button-small"
          autofocus
          onClick={() => dialog.close()}
        >
          {t.close}
        </button>
      </header>
      <Show when={mode() !== "links"}>
        <p class="mt-5 text-sm leading-relaxed text-muted">
          {mode() === "share" ? t.shareHint : t.retentionHint}
        </p>
        <div class="mt-5">
          <ExpiryPicker
            locale={props.locale}
            value={choice()}
            onChange={(value) => {
              setChoice(value);
              setConfirmDeletion(false);
            }}
            allowFollow={mode() === "share"}
            disabled={busy()}
          />
        </div>
        <p class="mt-3 text-xs leading-relaxed text-muted">
          {t.fileExpires}: {displayDate(row()?.expires_at ?? null)}
        </p>
        <Show when={deleting() && mode() === "expiry"}>
          <label class="mt-4 flex items-start gap-3 rounded-md border border-red-200 bg-red-50 p-3 text-sm text-red-900">
            <input
              class="mt-1"
              type="checkbox"
              checked={confirmDeletion()}
              onChange={(e) => setConfirmDeletion(e.currentTarget.checked)}
            />
            {t.deletionWarning}
          </label>
        </Show>
        <Show when={created()}>
          {(value) => (
            <div class="share-result mt-5">
              <label class="control-label">
                {t.shareCreated}
                <input
                  class="field w-full"
                  readonly
                  value={value().url}
                  onFocus={(e) => e.currentTarget.select()}
                />
              </label>
              <p class="mt-2 text-xs text-muted">
                {t.expires}: {displayDate(value().expires_at)}
              </p>
              <button
                class="button mt-3"
                onClick={() => void copy(value().url)}
              >
                {t.copy}
              </button>
            </div>
          )}
        </Show>
        <div class="mt-6 flex justify-end">
          <button
            class="button button-primary"
            disabled={busy() || !!created()}
            onClick={() => void save()}
          >
            {busy()
              ? t.working
              : mode() === "share"
                ? t.createShare
                : deleting()
                  ? t.remove
                  : t.saveExpiry}
          </button>
        </div>
      </Show>
      <Show when={mode() === "links"}>
        <p class="mt-5 text-sm leading-relaxed text-muted">{t.cleanupHint}</p>
        <Show
          when={shares().length > 0}
          fallback={
            <p class="py-8 text-sm text-muted">
              {busy() ? t.working : error() ? "" : t.noShares}
            </p>
          }
        >
          <ul class="mt-5 space-y-3">
            <For each={shares()}>
              {(share) => (
                <li class="rounded-lg border border-line p-4">
                  <div class="flex flex-wrap items-center justify-between gap-2">
                    <span class="badge bg-slate-100 text-ink">
                      {shareStatus(share)}
                    </span>
                    <span class="text-xs text-muted">
                      {t.created}: {formatDate(share.created_at, props.locale)}
                    </span>
                  </div>
                  <label class="control-label mt-3">
                    {t.share}
                    <input
                      class="field w-full text-sm"
                      readonly
                      value={safeDownloadUrl(share.url) ?? ""}
                      onFocus={(e) => e.currentTarget.select()}
                    />
                  </label>
                  <p class="mt-2 text-xs text-muted">
                    {t.expires}: {displayDate(share.expires_at)}
                    {share.follows_upload ? ` · ${t.untilUploadExpires}` : ""}
                  </p>
                  <div class="mt-3 flex flex-wrap gap-2">
                    <button
                      class="button button-small"
                      disabled={!safeDownloadUrl(share.url)}
                      onClick={() => void copy(share.url)}
                    >
                      {t.copy}
                    </button>
                    <button
                      class="button button-small text-red-800"
                      disabled={
                        busy() ||
                        share.status === "Revoked" ||
                        share.status === "Expired"
                      }
                      onClick={() => void revoke(share)}
                    >
                      {t.revoke}
                    </button>
                  </div>
                </li>
              )}
            </For>
          </ul>
        </Show>
        <div class="mt-4 flex flex-wrap justify-between gap-2">
          <button
            class="button button-small"
            disabled={busy()}
            onClick={() => void loadShares(after())}
          >
            {t.refresh}
          </button>
          <div class="flex gap-2">
            <Show when={after()}>
              <button
                class="button button-small"
                disabled={busy()}
                onClick={() => void loadShares()}
              >
                {t.first}
              </button>
            </Show>
            <Show when={next()}>
              {(cursor) => (
                <button
                  class="button button-small"
                  disabled={busy()}
                  onClick={() => void loadShares(cursor())}
                >
                  {t.next}
                </button>
              )}
            </Show>
          </div>
        </div>
      </Show>
      <p class="mt-4 text-sm text-red-800" role="alert">
        {error()}
      </p>
      <p class="mt-2 text-sm text-muted" role="status" aria-live="polite">
        {notice()}
      </p>
    </dialog>
  );
}
