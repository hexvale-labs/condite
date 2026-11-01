/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { Button } from "@components/Button";
import { Heading } from "@components/Heading";
import {
    openModal,
    React,
    useEffect,
    useRef,
    useState
} from "@webpack/common";

import { ErrorConfirmModal, ErrorMessage } from "./errors";
import { useTranslation } from "./i18n";
import { safeUrl } from "./routing";
import { createClient, uploadFile } from "./sdk";
import { settings } from "./settings";
import { type Expiry, expiryDate, ExpiryPicker, openSharing } from "./sharing";
import type { ListUploadsResponse, Upload } from "./wire";

export function bytes(value: bigint, locale = "en") {
    const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let divisor = 1n;
    let unit = 0;
    while (value >= divisor * 1024n && unit < units.length - 1) {
        divisor *= 1024n;
        unit++;
    }
    // Convert only after scaling, so the original u64 never passes through Number.
    const scaled = Number((value * 10n + divisor / 2n) / divisor) / 10;
    return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(scaled)} ${units[unit]}`;
}

interface Activity {
    name: string;
    stage: string;
    error?: unknown;
}

export function FileManager({ onClose }: { onClose(): void }) {
    const { t, locale, dir } = useTranslation();
    const { apiBase, apiKey } = settings.use(["apiBase", "apiKey"]);
    const [page, setPage] = useState<ListUploadsResponse>({
        uploads: [],
        next_after: null
    });
    const [after, setAfter] = useState<string | null>(null);
    const [history, setHistory] = useState<(string | null)[]>([]);
    const [busy, setBusy] = useState(false);
    const [loaded, setLoaded] = useState(false);
    const [error, setError] = useState<unknown>(null);
    const [notice, setNotice] = useState("");
    const [filter, setFilter] = useState("all");
    const [query, setQuery] = useState("");
    const [view, setView] = useState<"list" | "grid">("list");
    const [link, setLink] = useState<{ url: string; expires: string | null }>();
    const [retention, setRetention] = useState<Expiry>(() => settings.store.retentionDays === 0 ? "never" : String(settings.store.retentionDays ?? 14) as Expiry);
    const [retentionDate, setRetentionDate] = useState("");
    const [activity, setActivity] = useState<Activity[]>([]);
    const picker = useRef<HTMLInputElement>(null);
    const live = useRef(true);
    const locked = useRef(false);
    const generation = useRef(0);
    useEffect(() => {
        live.current = true;
        return () => {
            live.current = false;
            generation.current++;
        };
    }, []);

    async function run(
        operation: (
            sdk: ReturnType<typeof createClient>,
            current: () => boolean
        ) => Promise<void>
    ) {
        if (locked.current) return;
        locked.current = true;
        setBusy(true);
        setError("");
        setNotice("");
        const version = generation.current;
        const current = () => live.current && version === generation.current;
        try {
            await operation(createClient(), current);
        } catch (cause) {
            if (current()) setError(cause);
        } finally {
            locked.current = false;
            if (live.current) setBusy(false);
        }
    }

    function refresh() {
        void run(async (sdk, current) => {
            const result = await sdk.listUploads({ limit: 50, after });
            if (current()) {
                setPage(result);
                setLoaded(true);
            }
        });
    }
    useEffect(() => {
        generation.current++;
        setAfter(null);
        setHistory([]);
        setPage({ uploads: [], next_after: null });
        setLoaded(false);
        setLink(undefined);
        // This effect normally runs unlocked. An endpoint switch invalidates in-flight responses.
        let cancelled = false;
        const load = async () => {
            setBusy(true);
            try {
                const result = await createClient().listUploads({ limit: 50 });
                if (!cancelled) {
                    setPage(result);
                    setLoaded(true);
                    setError("");
                }
            } catch (cause) {
                if (!cancelled) setError(cause);
            } finally {
                if (!cancelled) setBusy(false);
            }
        };
        void load();
        return () => {
            cancelled = true;
        };
    }, [apiBase, apiKey]);

    async function navigate(cursor: string | null, back = false) {
        await run(async (sdk, current) => {
            const result = await sdk.listUploads({ limit: 50, after: cursor });
            if (!current()) return;
            setHistory(previous =>
                back ? previous.slice(0, -1) : [...previous, after]
            );
            setAfter(cursor);
            setPage(result);
            setLink(undefined);
        });
    }

    function operate(row: Upload, kind: "confirm" | "link") {
        void run(async (sdk, current) => {
            const value = kind === "link" ? await sdk.download(row.id) : await sdk.confirm(row.id);
            if (!current()) return;
            if (kind === "link")
                setLink({
                    url: safeUrl((value as { uri: string }).uri),
                    expires: (value as { expires_at: string }).expires_at
                });
            else setNotice(t.confirmedMessage);
            const result = await sdk.listUploads({ limit: 50, after });
            if (current()) setPage(result);
        });
    }

    function askDelete(row: Upload) {
        const endpoint = apiBase;
        const key = apiKey;
        const sdk = createClient();
        openModal(props => (
            <ErrorConfirmModal
                {...props}
                title={t.deleteTitle}
                confirmText={t.remove}
                cancelText={t.cancel}
                onConfirm={async () => {
                    if (settings.store.apiBase !== endpoint || settings.store.apiKey !== key)
                        throw new Error(t.error);
                    await sdk.delete(row.id);
                    if (
                        live.current &&
                        settings.store.apiBase === endpoint
                    ) {
                        setPage(previous => ({
                            ...previous,
                            uploads: previous.uploads.filter(
                                item => item.id !== row.id
                            )
                        }));
                        setNotice(t.deleted);
                        setLink(undefined);
                    }
                }}
            >
                <p className="condite-break">{row.filename}</p>
                <p>{t.deleteHint}</p>
            </ErrorConfirmModal>
        ));
    }

    function upload(files: File[]) {
        if (!files.length) return;
        void run(async (sdk, current) => {
            setActivity(
                files.map(file => ({ name: file.name, stage: "preparing" }))
            );
            const update = (index: number, changes: Partial<Activity>) => {
                if (current())
                    setActivity(items =>
                        items.map((item, i) =>
                            i === index ? { ...item, ...changes } : item
                        )
                    );
            };
            for (const [index, file] of files.entries()) {
                if (!current()) break;
                try {
                    await uploadFile(file, file.name, stage =>
                        update(index, { stage }), { expires_at: expiryDate(retention, retentionDate) }
                    );
                } catch (cause) {
                    update(index, { stage: "failed", error: cause });
                }
            }
            const result = await sdk.listUploads({ limit: 50 });
            if (current()) {
                setPage(result);
                setAfter(null);
                setHistory([]);
            }
            if (picker.current) picker.current.value = "";
        });
    }

    const rows = page.uploads.filter(
        row =>
            (filter === "all" || row.status === filter) &&
            row.filename.toLocaleLowerCase().includes(query.toLocaleLowerCase())
    );
    const actions = (row: Upload) => (
        <div className="condite-actions">
            <Button
                size="small"
                variant="secondary"
                disabled={busy}
                onClick={() =>
                    operate(row, row.status === "Pending" ? "confirm" : "link")
                }
            >
                {row.status === "Pending" ? t.confirm : t.download}
            </Button>
            {row.status === "Confirmed" && <>
                <Button size="small" variant="secondary" disabled={busy || row.deletion_pending} onClick={() => openSharing(row, "share", refresh)}>{t.share}</Button>
                <Button size="small" variant="secondary" disabled={busy} onClick={() => openSharing(row, "manage", refresh)}>{t.manageLinks}</Button>
            </>}
            <Button size="small" variant="secondary" disabled={busy || row.deletion_pending} onClick={() => openSharing(row, "expiry", refresh)}>{t.retention}</Button>
            <Button
                size="small"
                variant="dangerSecondary"
                disabled={busy}
                onClick={() => askDelete(row)}
            >
                {t.remove}
            </Button>
        </div>
    );

    return (
        <section
            lang={locale}
            dir={dir}
            className="condite-manager"
            aria-label={t.uploads}
        >
            <header className="condite-pane-header">
                <Heading tag="h1">{t.uploads}</Heading>
                <Button variant="secondary" size="small" onClick={onClose}>
                    {t.backDiscord}
                </Button>
            </header>
            <div className="condite-manager-body">
                <div className="condite-toolbar">
                    <Button
                        disabled={busy}
                        onClick={() => picker.current?.click()}
                    >
                        {t.upload}
                    </Button>
                    <input
                        ref={picker}
                        type="file"
                        hidden
                        multiple
                        onChange={event =>
                            upload(Array.from(event.target.files ?? []))
                        }
                    />
                    <Button
                        variant="secondary"
                        disabled={busy}
                        onClick={refresh}
                    >
                        {t.refresh}
                    </Button>
                    <Button
                        variant="secondary"
                        disabled={busy}
                        onClick={() =>
                            void run(async (sdk, current) => {
                                const health = await sdk.health();
                                if (current())
                                    setNotice(
                                        health.healthy
                                            ? `${t.healthy} · ${health.runtime}`
                                            : t.unhealthy
                                    );
                            })
                        }
                    >
                        {t.connection}
                    </Button>
                    <span className="condite-muted">{t.fileLimit}</span>
                </div>
                <ExpiryPicker value={retention} onChange={setRetention} date={retentionDate} onDate={setRetentionDate} />
                <div role="status" aria-live="polite">
                    {busy ? t.working : notice}
                </div>
                {!!error && (
                    <div role="alert" className="condite-error">
                        <ErrorMessage error={error} />
                        <Button
                            variant="secondary"
                            size="small"
                            disabled={busy}
                            onClick={refresh}
                        >
                            {t.retry}
                        </Button>
                    </div>
                )}
                {link && (
                    <section className="condite-link" aria-label={t.link}>
                        <strong>{t.linkReady}</strong>
                        <p>
                            {t.expires}:{" "}
                            {link.expires ? new Date(link.expires).toLocaleString(locale, { timeZone: "UTC" }) + " UTC" : t.never}
                        </p>
                        <input
                            aria-label={t.link}
                            readOnly
                            value={link.url}
                            onFocus={event => event.target.select()}
                        />
                        <div className="condite-actions">
                            <Button
                                size="small"
                                onClick={() => {
                                    void navigator.clipboard
                                        .writeText(link.url)
                                        .then(
                                            () => { setError(null); setNotice(t.copied); },
                                            cause => { setError(cause); setNotice(t.copyFailed); }
                                        );
                                }}
                            >
                                {t.copy}
                            </Button>
                            <a href={link.url} target="_blank" rel="noreferrer">
                                {t.open}
                            </a>
                            <Button
                                variant="secondary"
                                size="small"
                                onClick={() => setLink(undefined)}
                            >
                                {t.close}
                            </Button>
                        </div>
                    </section>
                )}
                {!!activity.length && (
                    <ul className="condite-activity" aria-label={t.uploads}>
                        {activity.map((item, i) => (
                            <li key={i}>
                                <strong className="condite-break">
                                    {item.name}
                                </strong>
                                <span>
                                    {
                                        t[
                                            item.stage as
                                                | "preparing"
                                                | "uploading"
                                                | "confirming"
                                                | "complete"
                                                | "failed"
                                        ]
                                    }
                                </span>
                                {!!item.error && (
                                    <ErrorMessage error={item.error} />
                                )}
                            </li>
                        ))}
                    </ul>
                )}
                <div className="condite-toolbar condite-filters">
                    <input
                        type="search"
                        aria-label={t.search}
                        placeholder={t.search}
                        value={query}
                        onChange={event => setQuery(event.target.value)}
                    />
                    <select
                        aria-label={t.status}
                        value={filter}
                        onChange={event => setFilter(event.target.value)}
                    >
                        <option value="all">{t.all}</option>
                        <option value="Pending">{t.pending}</option>
                        <option value="Confirmed">{t.confirmed}</option>
                    </select>
                    <div className="condite-view-toggle" role="group" aria-label={t.view}>
                        <Button
                            size="small"
                            variant="secondary"
                            aria-pressed={view === "list"}
                            onClick={() => setView("list")}
                        >
                            {t.list}
                        </Button>
                        <Button
                            size="small"
                            variant="secondary"
                            aria-pressed={view === "grid"}
                            onClick={() => setView("grid")}
                        >
                            {t.grid}
                        </Button>
                    </div>
                </div>
                {!rows.length && !busy && !error && (
                    <div className="condite-empty">
                        <Heading tag="h2">
                            {!loaded
                                ? t.failedLoad
                                : page.uploads.length
                                  ? t.noMatches
                                  : t.empty}
                        </Heading>
                        <p>{!loaded ? t.error : t.emptyHint}</p>
                    </div>
                )}
                {!!rows.length &&
                    (view === "list" ? (
                        <div className="condite-table-wrap">
                            <table className="condite-table">
                                <thead>
                                    <tr>
                                        <th>{t.name}</th>
                                        <th>{t.size}</th>
                                        <th>{t.status}</th>
                                        <th>{t.created}</th>
                                        <th>{t.actions}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {rows.map(row => (
                                        <tr key={row.id}>
                                            <td className="condite-file-name">
                                                <strong>{row.filename}</strong>
                                                <small>{row.mime_type} · {row.visibility === "Shared" ? t.shared : t.private} · {row.deletion_pending ? t.cleanupPending : row.expires_at ? new Date(row.expires_at).toLocaleString(locale, { timeZone: "UTC" }) + " UTC" : t.never}</small>
                                            </td>
                                            <td
                                                title={`${row.size_bytes} ${t.bytes}`}
                                            >
                                                {bytes(row.size_bytes, locale)}
                                            </td>
                                            <td>
                                                <span className="condite-status">
                                                    {row.status === "Pending"
                                                        ? t.pending
                                                        : t.confirmed}
                                                </span>
                                            </td>
                                            <td>
                                                {new Date(
                                                    row.created_at
                                                ).toLocaleString(locale, {
                                                    timeZone: "UTC"
                                                })}
                                            </td>
                                            <td>{actions(row)}</td>
                                        </tr>
                                    ))}
                                </tbody>
                            </table>
                        </div>
                    ) : (
                        <ul className="condite-grid">
                            {rows.map(row => (
                                <li key={row.id}>
                                    <span className="condite-status">
                                        {row.status === "Pending"
                                            ? t.pending
                                            : t.confirmed}
                                    </span>
                                    <Heading tag="h3" className="condite-break">
                                        {row.filename}
                                    </Heading>
                                    <p>
                                        {bytes(row.size_bytes, locale)} ·{" "}
                                        {row.mime_type} · {row.visibility === "Shared" ? t.shared : t.private} · {row.deletion_pending ? t.cleanupPending : row.expires_at ? new Date(row.expires_at).toLocaleString(locale, { timeZone: "UTC" }) + " UTC" : t.never}
                                    </p>
                                    <p>
                                        {new Date(
                                            row.created_at
                                        ).toLocaleString(locale, {
                                            timeZone: "UTC"
                                        })}
                                    </p>
                                    {actions(row)}
                                </li>
                            ))}
                        </ul>
                    ))}
                <footer className="condite-toolbar">
                    <span className="condite-muted">
                        {t.shown}:{" "}
                        {new Intl.NumberFormat(locale).format(rows.length)} ·{" "}
                        {t.pageNote}
                    </span>
                    <Button
                        variant="secondary"
                        size="small"
                        disabled={busy || !history.length}
                        onClick={() =>
                            void navigate(history.at(-1) ?? null, true)
                        }
                    >
                        {t.previous}
                    </Button>
                    <Button
                        variant="secondary"
                        size="small"
                        disabled={busy || !page.next_after}
                        onClick={() => void navigate(page.next_after)}
                    >
                        {t.next}
                    </Button>
                </footer>
                <p className="condite-muted">
                    {t.cleanupHint} {t.uploadAgain}
                </p>
            </div>
        </section>
    );
}
