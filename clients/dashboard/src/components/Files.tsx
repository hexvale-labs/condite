import {
  createMemo,
  createSignal,
  For,
  onCleanup,
  onSettled,
  Show,
} from "solid-js";
import type { Upload, ListUploadsResponse } from "condite-sdk-wasm";
import { browserClient } from "../lib/client";
import { formatBytes, formatDate, safeDownloadUrl } from "../lib/format";
import { messages, type Locale } from "../i18n";
import FileIcon from "./FileIcon";
import FileAccess, { type FileAccessHandle } from "./FileAccess";
import ExpiryPicker from "./ExpiryPicker";
import { defaultExpiry, resolveExpiry, type ExpiryChoice } from "../lib/expiry";

type Props = {
  locale: Locale;
  initial: ListUploadsResponse | null;
  failed: boolean;
  view: "list" | "grid";
  after: string | null;
};
type Activity = {
  name: string;
  stage: "preparing" | "uploading" | "confirming" | "complete" | "failed";
  requestId?: string;
  error?: string;
};

export default function Files(props: Props) {
  const t = messages(props.locale);
  const [page, setPage] = createSignal(() => props.initial);
  const [loadFailed, setLoadFailed] = createSignal(() => props.failed);
  const [view, setView] = createSignal(() => props.view);
  const [filter, setFilter] = createSignal("all");
  const [ready, setReady] = createSignal(false);
  const [busy, setBusy] = createSignal(false);
  const [feedback, setFeedback] = createSignal("");
  const [activities, setActivities] = createSignal<Activity[]>([]);
  const [deleteError, setDeleteError] = createSignal("");
  const [deleting, setDeleting] = createSignal<Upload | null>(null);
  const [uploadExpiry, setUploadExpiry] =
    createSignal<ExpiryChoice>(defaultExpiry());
  const [showUploadExpiry, setShowUploadExpiry] = createSignal(false);
  let access!: FileAccessHandle;
  const [download, setDownload] = createSignal<{
    url: string;
    expires: string;
  } | null>(null);
  const rows = createMemo(() =>
    (page()?.uploads ?? []).filter(
      (row) => filter() === "all" || row.status === filter(),
    ),
  );
  let picker!: HTMLInputElement;
  let uploadButton!: HTMLButtonElement;
  let dialog!: HTMLDialogElement;
  let disposed = false;
  onCleanup(() => {
    disposed = true;
  });
  onSettled(() => {
    setReady(true);
  });

  const href = (after: string | null) => {
    const search = new URLSearchParams();
    if (view() === "grid") search.set("view", "grid");
    if (after) search.set("after", after);
    return `/${props.locale}/${search.size ? `?${search}` : ""}`;
  };
  function changeView(next: "list" | "grid") {
    setView(next);
    const url = new URL(window.location.href);
    if (next === "grid") url.searchParams.set("view", next);
    else url.searchParams.delete("view");
    history.replaceState(null, "", url);
    for (const link of document.querySelectorAll<HTMLAnchorElement>(
      "a[hreflang]",
    )) {
      const target = new URL(link.href);
      target.search = url.search;
      link.href = target.href;
    }
  }
  async function refreshData() {
    const sdk = await browserClient();
    const result = await sdk.listUploads({
      limit: 50,
      after: props.after ?? undefined,
    });
    if (!disposed) {
      setPage(result);
      setLoadFailed(false);
    }
  }
  async function refresh() {
    if (busy()) return;
    setBusy(true);
    setFeedback("");
    try {
      await refreshData();
    } catch {
      if (!disposed) {
        setLoadFailed(true);
        setFeedback(t.failedLoad);
      }
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  async function operate(
    row: Upload,
    operation: "confirm" | "link" | "delete",
  ) {
    if (busy()) return;
    setBusy(true);
    setFeedback("");
    setDeleteError("");
    try {
      const sdk = await browserClient();
      if (operation === "delete") {
        await sdk.delete(row.id);
        if (disposed) return;
        // Remove only after confirmed deletion; retain the row when deletion fails.
        setPage((previous) =>
          previous
            ? {
                ...previous,
                uploads: previous.uploads.filter((item) => item.id !== row.id),
              }
            : previous,
        );
        dialog.close();
        setDeleting(null);
        setFeedback(t.deleted);
        queueMicrotask(() => uploadButton.focus());
      } else {
        if (operation === "link") {
          const result = await sdk.download(row.id);
          const url = safeDownloadUrl(result.uri);
          if (!url) throw new Error("Invalid download URL");
          setDownload({ url, expires: result.expires_at });
        } else {
          await sdk.confirm(row.id);
          if (!disposed) setFeedback(t.confirmedMessage);
        }
        await refreshData();
      }
    } catch {
      if (!disposed) {
        setFeedback(t.error);
        if (operation === "delete") setDeleteError(t.error);
      }
    } finally {
      if (!disposed) setBusy(false);
    }
  }
  function askDelete(row: Upload) {
    setDeleteError("");
    setDeleting(row);
    dialog.showModal();
  }
  async function copyLink() {
    try {
      await navigator.clipboard.writeText(download()!.url);
      setFeedback(t.copied);
    } catch {
      setFeedback(t.copyFailed);
    }
  }
  async function upload(files: File[]) {
    if (busy() || !files.length) return;
    setBusy(true);
    setFeedback("");
    setActivities(
      files.map((file) => ({ name: file.name, stage: "preparing" })),
    );
    const update = (index: number, changes: Partial<Activity>) => {
      if (!disposed)
        setActivities((previous) =>
          previous.map((item, i) =>
            i === index ? { ...item, ...changes } : item,
          ),
        );
    };
    try {
      const sdk = await browserClient();
      const expires = resolveExpiry(uploadExpiry());
      if (typeof expires === "string" && Date.parse(expires) <= Date.now())
        throw new Error(t.invalidExpiry);
      for (const [index, file] of files.entries()) {
        if (disposed) break;
        const nameSize = new TextEncoder().encode(file.name).length;
        if (!nameSize || nameSize > 255) {
          update(index, {
            stage: "failed",
            error: t.invalidName,
          });
          continue;
        }
        try {
          const signed = await sdk.presign({
            filename: file.name,
            mime_type: file.type || "application/octet-stream",
            size_bytes: BigInt(file.size),
            ...(expires === undefined
              ? {}
              : { retention: { expires_at: expires } }),
          });
          update(index, { stage: "uploading", requestId: signed.request_id });
          await sdk.putUpload(signed, new Uint8Array(await file.arrayBuffer()));
          update(index, { stage: "confirming" });
          await sdk.confirm(signed.request_id);
          update(index, { stage: "complete" });
        } catch {
          update(index, {
            stage: "failed",
            error: `${t.failedUpload} ${t.uploadAgain}`,
          });
        }
      }
      await refreshData();
      if (!disposed)
        setFeedback(
          activities().some((item) => item.stage === "failed")
            ? t.failed
            : t.complete,
        );
    } catch {
      if (!disposed) {
        setFeedback(t.error);
        setActivities((items) =>
          items.map((item) =>
            item.stage === "complete" || item.stage === "failed"
              ? item
              : { ...item, stage: "failed", error: t.error },
          ),
        );
      }
    } finally {
      if (!disposed) {
        setBusy(false);
        picker.value = "";
      }
    }
  }
  const blocked = () => !ready() || busy();
  const Status = (p: { row: Upload }) => (
    <span
      class={[
        "badge",
        p.row.status === "Confirmed"
          ? "bg-emerald-50 text-emerald-800"
          : "bg-amber-50 text-amber-900",
      ]}
    >
      <span aria-hidden="true">{p.row.status === "Confirmed" ? "✓" : "◷"}</span>
      {p.row.status === "Confirmed" ? t.confirmed : t.pending}
    </span>
  );
  const AccessStatus = (p: { row: Upload }) => (
    <div class="mt-2 space-y-1 text-xs text-muted">
      <p>
        {p.row.deletion_pending
          ? t.cleanupPending
          : p.row.visibility === "Shared"
            ? t.shared
            : t.private}
      </p>
      <p>
        {t.fileExpires}:{" "}
        {p.row.expires_at
          ? `${formatDate(p.row.expires_at, props.locale)} UTC`
          : t.never}
      </p>
    </div>
  );
  const Actions = (p: { row: Upload }) => (
    <div class="file-actions">
      <Show when={p.row.status === "Confirmed"}>
        <button
          class="button button-small"
          disabled={blocked() || p.row.deletion_pending}
          onClick={() => access.open(p.row, "share")}
        >
          {t.share}
        </button>
        <button
          class="button button-small"
          disabled={blocked()}
          onClick={() => access.open(p.row, "links")}
        >
          {t.manageLinks}
        </button>
      </Show>
      <button
        class="button button-small"
        disabled={blocked() || p.row.deletion_pending}
        onClick={() => access.open(p.row, "expiry")}
      >
        {t.retention}
      </button>
      <button
        class="button button-small"
        disabled={blocked()}
        onClick={() =>
          void operate(p.row, p.row.status === "Pending" ? "confirm" : "link")
        }
      >
        {p.row.status === "Pending" ? t.confirm : t.download}
      </button>
      <button
        class="button button-small text-red-800"
        disabled={blocked()}
        onClick={() => askDelete(p.row)}
      >
        {t.remove}
      </button>
    </div>
  );

  return (
    <section aria-label={t.files} class="files-workspace">
      <div class="flex flex-wrap items-center justify-between gap-4">
        <div>
          <button
            ref={(el) => {
              uploadButton = el;
            }}
            class="button button-primary"
            disabled={blocked()}
            onClick={() => picker.click()}
          >
            <span aria-hidden="true">＋</span>
            {t.upload}
          </button>
          <input
            ref={(el) => {
              picker = el;
            }}
            type="file"
            multiple
            hidden
            tabindex="-1"
            aria-label={t.upload}
            disabled={blocked()}
            onChange={(event) =>
              void upload(Array.from(event.currentTarget.files ?? []))
            }
          />
          <button
            class="ms-2 button"
            disabled={blocked()}
            aria-expanded={showUploadExpiry() ? "true" : "false"}
            onClick={() => setShowUploadExpiry((value) => !value)}
          >
            {t.uploadRetention}
          </button>
          <p class="mt-3 text-xs leading-relaxed text-muted">
            {t.uploadHint} · {t.defaultRetention}
          </p>
        </div>
        <div class="flex flex-wrap items-center gap-3">
          <button
            class="button"
            disabled={blocked()}
            onClick={() => void refresh()}
          >
            {busy() ? t.working : t.refresh}
          </button>
          <div
            role="group"
            aria-label={t.view}
            class="flex gap-1 rounded-lg border border-line bg-white p-1"
          >
            <button
              class={[
                "button border-0",
                view() === "list" && "bg-slate-100 text-accent",
              ]}
              aria-label={t.list}
              aria-pressed={view() === "list" ? "true" : "false"}
              disabled={!ready()}
              onClick={() => changeView("list")}
            >
              <svg
                width="18"
                height="18"
                viewBox="0 0 20 20"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                aria-hidden="true"
              >
                <path d="M6 5h11M6 10h11M6 15h11M2 5h1m-1 5h1m-1 5h1" />
              </svg>
            </button>
            <button
              class={[
                "button border-0",
                view() === "grid" && "bg-slate-100 text-accent",
              ]}
              aria-label={t.grid}
              aria-pressed={view() === "grid" ? "true" : "false"}
              disabled={!ready()}
              onClick={() => changeView("grid")}
            >
              <svg
                width="18"
                height="18"
                viewBox="0 0 20 20"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
                aria-hidden="true"
              >
                <rect x="3" y="3" width="5" height="5" />
                <rect x="12" y="3" width="5" height="5" />
                <rect x="3" y="12" width="5" height="5" />
                <rect x="12" y="12" width="5" height="5" />
              </svg>
            </button>
          </div>
        </div>
      </div>
      <Show when={showUploadExpiry()}>
        <div class="mt-5 max-w-lg rounded-xl border border-line bg-white p-5">
          <ExpiryPicker
            locale={props.locale}
            value={uploadExpiry()}
            onChange={setUploadExpiry}
            allowDefault
            disabled={blocked()}
          />
        </div>
      </Show>
      <div class="feedback" role="status" aria-live="polite" aria-atomic="true">
        {feedback()}
      </div>
      <Show when={download()}>
        {(value) => (
          <section
            class="mb-5 rounded-lg border border-blue-200 bg-blue-50 p-4"
            aria-label={t.linkReady}
          >
            <div class="flex flex-wrap items-center justify-between gap-3">
              <h2 class="font-semibold">{t.linkReady}</h2>
              <button
                class="button button-small"
                onClick={() => setDownload(null)}
              >
                {t.close}
              </button>
            </div>
            <p class="mt-2 text-sm">{t.linkHint}</p>
            <p class="mt-1 text-sm text-muted">
              {t.expires}: {formatDate(value().expires, props.locale)} UTC
            </p>
            <input
              class="field mt-3 w-full"
              aria-label={t.linkReady}
              readonly
              value={value().url}
              onFocus={(event) => event.currentTarget.select()}
            />
            <div class="mt-3 flex flex-wrap gap-2">
              <button class="button" onClick={() => void copyLink()}>
                {t.copy}
              </button>
              <a
                class="button"
                href={value().url}
                target="_blank"
                rel="noopener noreferrer"
              >
                {t.open}
              </a>
            </div>
          </section>
        )}
      </Show>
      <Show when={activities().length > 0}>
        <section
          class="mb-5 rounded-lg border border-line bg-white p-4"
          aria-label={t.uploads}
        >
          <h2 class="text-sm font-semibold">{t.uploads}</h2>
          <ul class="mt-3 space-y-3">
            <For each={activities()}>
              {(item) => (
                <li class="text-sm">
                  <div class="flex flex-wrap justify-between gap-2">
                    <span class="min-w-0 break-all font-medium">
                      {item.name}
                    </span>
                    <span>{t[item.stage]}</span>
                  </div>
                  <Show when={item.error}>
                    <p class="mt-1 text-red-800">{item.error}</p>
                  </Show>
                  <Show when={item.requestId && item.stage === "failed"}>
                    <p class="mt-1 break-all text-muted">
                      {t.pendingId}: {item.requestId}
                    </p>
                  </Show>
                </li>
              )}
            </For>
          </ul>
        </section>
      </Show>
      <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
        <p class="text-sm text-muted">
          {t.shown}:{" "}
          <span class="font-medium text-ink">
            {new Intl.NumberFormat(props.locale).format(
              page()?.uploads.length ?? 0,
            )}
          </span>
        </p>
        <label class="flex flex-wrap items-center gap-2 text-sm text-muted">
          {t.filter}
          <select
            class="field"
            value={filter()}
            onChange={(event) => setFilter(event.currentTarget.value)}
          >
            <option value="all">{t.all}</option>
            <option value="Pending">{t.pending}</option>
            <option value="Confirmed">{t.confirmed}</option>
          </select>
        </label>
      </div>
      <Show when={loadFailed() && rows().length > 0}>
        <div class="mb-4 rounded-md border border-red-200 bg-red-50 p-4 text-sm text-red-900">
          {t.failedLoad}{" "}
          <button
            class="underline"
            disabled={blocked()}
            onClick={() => void refresh()}
          >
            {t.retry}
          </button>
        </div>
      </Show>
      <Show
        when={rows().length > 0}
        fallback={
          <div class="rounded-lg border border-dashed border-line bg-white px-6 py-16 text-center">
            <FileIcon />
            <h2 class="mt-4 text-lg font-semibold">
              {page()?.uploads.length
                ? t.noMatches
                : loadFailed()
                  ? t.failedLoad
                  : t.empty}
            </h2>
            <p class="mt-2 text-sm text-muted">
              {page()?.uploads.length
                ? t.pageNote
                : loadFailed()
                  ? t.error
                  : t.emptyHint}
            </p>
            <Show when={loadFailed()}>
              <button
                class="button mt-4"
                disabled={blocked()}
                onClick={() => void refresh()}
              >
                {t.retry}
              </button>
            </Show>
          </div>
        }
      >
        <Show
          when={view() === "list"}
          fallback={
            <ul
              aria-label={t.files}
              class="grid gap-4 sm:grid-cols-2 xl:grid-cols-3"
            >
              <For each={rows()} keyed={(row) => row.id}>
                {(row) => (
                  <li class="file-card min-w-0 rounded-xl border border-line bg-white p-5">
                    <div class="flex items-start justify-between gap-2">
                      <FileIcon />
                      <Status row={row()} />
                    </div>
                    <h2 class="mt-4 break-all text-sm font-semibold">
                      {row().filename}
                    </h2>
                    <p class="mt-1 break-all text-xs text-muted">
                      {row().mime_type}
                    </p>
                    <dl class="my-5 space-y-2 text-sm">
                      <div class="flex justify-between gap-3">
                        <dt class="text-muted">{t.size}</dt>
                        <dd
                          class="tabular-nums"
                          title={`${row().size_bytes} ${t.bytes}`}
                        >
                          {formatBytes(row().size_bytes, props.locale)}
                        </dd>
                      </div>
                      <div>
                        <dt class="text-xs text-muted">{t.created}</dt>
                        <dd class="mt-1 text-xs tabular-nums">
                          {formatDate(row().created_at, props.locale)}
                        </dd>
                      </div>
                    </dl>
                    <AccessStatus row={row()} />
                    <div class="mt-4">
                      <Actions row={row()} />
                    </div>
                  </li>
                )}
              </For>
            </ul>
          }
        >
          <div
            class="overflow-x-auto rounded-lg border border-line bg-white"
            role="region"
            aria-label={t.list}
            tabindex="0"
          >
            <table class="data-table w-full">
              <caption class="sr-only">{t.files}</caption>
              <thead class="bg-slate-50">
                <tr>
                  <th scope="col">{t.name}</th>
                  <th scope="col">{t.size}</th>
                  <th scope="col">{t.status}</th>
                  <th scope="col">{t.created}</th>
                  <th scope="col">{t.actions}</th>
                </tr>
              </thead>
              <tbody>
                <For each={rows()} keyed={(row) => row.id}>
                  {(row) => (
                    <tr class="file-row">
                      <td>
                        <div class="flex items-center gap-3">
                          <FileIcon />
                          <div class="min-w-0">
                            <span class="block break-all font-medium">
                              {row().filename}
                            </span>
                            <span class="mt-1 block break-all text-xs text-muted">
                              {row().mime_type}
                            </span>
                          </div>
                        </div>
                      </td>
                      <td
                        class="whitespace-nowrap tabular-nums"
                        title={`${row().size_bytes} ${t.bytes}`}
                      >
                        {formatBytes(row().size_bytes, props.locale)}
                      </td>
                      <td>
                        <Status row={row()} />
                        <AccessStatus row={row()} />
                      </td>
                      <td class="whitespace-nowrap text-xs text-muted tabular-nums">
                        {formatDate(row().created_at, props.locale)}
                      </td>
                      <td>
                        <Actions row={row()} />
                      </td>
                    </tr>
                  )}
                </For>
              </tbody>
            </table>
          </div>
        </Show>
      </Show>
      <footer class="mt-4 flex flex-wrap items-center justify-between gap-3 text-xs text-muted">
        <p>{t.pageNote}</p>
        <nav aria-label={t.files} class="flex gap-2">
          <Show when={props.after}>
            <a class="button button-small" href={href(null)}>
              {t.first}
            </a>
          </Show>
          <Show when={page()?.next_after}>
            {(cursor) => (
              <a class="button button-small" href={href(cursor())}>
                {t.next}
              </a>
            )}
          </Show>
        </nav>
      </footer>
      <p class="mt-8 max-w-prose text-xs leading-relaxed text-muted">
        {t.temporary}
      </p>
      <FileAccess
        locale={props.locale}
        onReady={(handle) => {
          access = handle;
        }}
        onChanged={refreshData}
        onDeleted={() => queueMicrotask(() => uploadButton.focus())}
      />
      <dialog
        ref={(el) => {
          dialog = el;
        }}
        class="file-dialog"
        aria-labelledby="delete-title"
        aria-describedby="delete-hint"
        onClose={() => setDeleting(null)}
      >
        <h2 id="delete-title" class="text-lg font-semibold">
          {t.deleteTitle}
        </h2>
        <p id="delete-hint" class="mt-2 text-sm leading-relaxed text-muted">
          {t.deleteHint}
        </p>
        <p class="mt-4 break-all text-sm font-medium">{deleting()?.filename}</p>
        <p class="mt-3 text-sm text-red-800" role="alert">
          {deleteError()}
        </p>
        <div class="mt-6 flex justify-end gap-2">
          <button
            class="button"
            autofocus
            disabled={busy()}
            onClick={() => dialog.close()}
          >
            {t.cancel}
          </button>
          <button
            class="button button-danger"
            disabled={busy()}
            onClick={() => {
              const row = deleting();
              if (row) void operate(row, "delete");
            }}
          >
            {busy() ? t.deleting : t.remove}
          </button>
        </div>
      </dialog>
    </section>
  );
}
