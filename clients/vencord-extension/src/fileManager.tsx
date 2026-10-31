// SPDX-License-Identifier: GPL-3.0-or-later
import { Button } from "@components/Button";
import { Heading } from "@components/Heading";
import {
    ConfirmModal,
    openModal,
    React,
    useEffect,
    useRef,
    useState
} from "@webpack/common";

import { safeUrl } from "./routing";
import { createClient, uploadFile } from "./sdk";
import { settings } from "./settings";
import type { ListUploadsResponse, Upload } from "./wire";

export function bytes(value: bigint) {
    if (value < 1024n) return `${value} B`;
    if (value < 1024n ** 2n) return `${value / 1024n} KiB`;
    return `${(value * 10n) / 1024n ** 2n / 10n}.${((value * 10n) / 1024n ** 2n) % 10n} MiB`;
}

export function errorText(error: unknown) {
    const value = error as Error & { request_id?: string; stage?: string };
    return `${value.message || "Request failed"}${value.request_id ? ` (upload ${value.request_id}${value.stage ? `, ${value.stage}` : ""})` : ""}`;
}

interface Activity {
    name: string;
    stage: string;
    error?: string;
}

export function FileManager({ onClose }: { onClose(): void }) {
    const { apiBase } = settings.use(["apiBase"]);
    const [page, setPage] = useState<ListUploadsResponse>({
        uploads: [],
        next_after: null
    });
    const [after, setAfter] = useState<string | null>(null);
    const [history, setHistory] = useState<(string | null)[]>([]);
    const [busy, setBusy] = useState(false);
    const [loaded, setLoaded] = useState(false);
    const [error, setError] = useState("");
    const [notice, setNotice] = useState("");
    const [filter, setFilter] = useState("all");
    const [query, setQuery] = useState("");
    const [view, setView] = useState<"list" | "grid">("list");
    const [link, setLink] = useState<{ url: string; expires: string }>();
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
            if (current()) setError(errorText(cause));
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
                if (!cancelled) setError(errorText(cause));
            } finally {
                if (!cancelled) setBusy(false);
            }
        };
        void load();
        return () => {
            cancelled = true;
        };
    }, [apiBase]);

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
            const value = await sdk.confirm(row.id);
            if (!current()) return;
            if (kind === "link")
                setLink({
                    url: safeUrl(value.public_uri),
                    expires: value.expires_at
                });
            else setNotice(`Confirmed ${row.filename}`);
            const result = await sdk.listUploads({ limit: 50, after });
            if (current()) setPage(result);
        });
    }

    function askDelete(row: Upload) {
        const endpoint = apiBase;
        openModal(props => (
            <ConfirmModal
                {...props}
                title="Delete file?"
                confirmText="Delete file"
                cancelText="Keep file"
                onConfirm={async setModalError => {
                    try {
                        if (settings.store.apiBase !== endpoint)
                            throw new Error(
                                "API base changed. Close this dialog and try again."
                            );
                        await createClient().delete(row.id);
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
                            setNotice(`Deleted ${row.filename}`);
                            setLink(undefined);
                        }
                    } catch (cause) {
                        setModalError(errorText(cause));
                        throw cause;
                    }
                }}
            >
                <p className="condite-break">{row.filename}</p>
                <p>The stored file will be removed. This cannot be undone.</p>
            </ConfirmModal>
        ));
    }

    function upload(files: File[]) {
        if (!files.length) return;
        void run(async (sdk, current) => {
            setActivity(
                files.map(file => ({ name: file.name, stage: "Queued" }))
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
                        update(index, { stage })
                    );
                } catch (cause) {
                    update(index, { stage: "Failed", error: errorText(cause) });
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
                {row.status === "Pending" ? "Confirm" : "Get link"}
            </Button>
            <Button
                size="small"
                variant="dangerSecondary"
                disabled={busy}
                onClick={() => askDelete(row)}
            >
                Delete
            </Button>
        </div>
    );

    return (
        <section className="condite-manager" aria-label="Uploads">
            <header className="condite-pane-header">
                <Heading tag="h1">Uploads</Heading>
                <Button variant="secondary" size="small" onClick={onClose}>
                    Back to Discord
                </Button>
            </header>
            <div className="condite-manager-body">
                <div className="condite-toolbar">
                    <Button
                        disabled={busy}
                        onClick={() => picker.current?.click()}
                    >
                        Upload files
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
                        Refresh
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
                                            ? `Connected · ${health.runtime}`
                                            : "Service reports unhealthy"
                                    );
                            })
                        }
                    >
                        Check connection
                    </Button>
                    <span className="condite-muted">
                        Up to 100 MiB per file
                    </span>
                </div>
                <div role="status" aria-live="polite">
                    {busy ? "Working…" : notice}
                </div>
                {error && (
                    <div role="alert" className="condite-error">
                        {error}
                        <Button
                            variant="secondary"
                            size="small"
                            disabled={busy}
                            onClick={refresh}
                        >
                            Retry
                        </Button>
                    </div>
                )}
                {link && (
                    <section
                        className="condite-link"
                        aria-label="Download link"
                    >
                        <strong>Temporary download link</strong>
                        <p>Expires {new Date(link.expires).toLocaleString()}</p>
                        <input
                            aria-label="Download URL"
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
                                            () => setNotice("Link copied"),
                                            () =>
                                                setNotice(
                                                    "Select and copy the link above"
                                                )
                                        );
                                }}
                            >
                                Copy link
                            </Button>
                            <a href={link.url} target="_blank" rel="noreferrer">
                                Open file
                            </a>
                            <Button
                                variant="secondary"
                                size="small"
                                onClick={() => setLink(undefined)}
                            >
                                Dismiss
                            </Button>
                        </div>
                    </section>
                )}
                {!!activity.length && (
                    <ul
                        className="condite-activity"
                        aria-label="Upload progress"
                    >
                        {activity.map((item, i) => (
                            <li key={i}>
                                <strong className="condite-break">
                                    {item.name}
                                </strong>
                                <span>{item.stage}</span>
                                {item.error && (
                                    <p className="condite-error">
                                        {item.error}
                                    </p>
                                )}
                            </li>
                        ))}
                    </ul>
                )}
                <div className="condite-toolbar condite-filters">
                    <input
                        type="search"
                        aria-label="Search this page"
                        placeholder="Search this page"
                        value={query}
                        onChange={event => setQuery(event.target.value)}
                    />
                    <select
                        aria-label="File status"
                        value={filter}
                        onChange={event => setFilter(event.target.value)}
                    >
                        <option value="all">All files</option>
                        <option value="Pending">Pending</option>
                        <option value="Confirmed">Confirmed</option>
                    </select>
                    <div role="group" aria-label="File view">
                        <Button
                            size="small"
                            variant="secondary"
                            aria-pressed={view === "list"}
                            onClick={() => setView("list")}
                        >
                            List
                        </Button>
                        <Button
                            size="small"
                            variant="secondary"
                            aria-pressed={view === "grid"}
                            onClick={() => setView("grid")}
                        >
                            Grid
                        </Button>
                    </div>
                </div>
                {!rows.length && !busy && (
                    <div className="condite-empty">
                        <Heading tag="h2">
                            {!loaded
                                ? "Could not load files"
                                : page.uploads.length
                                  ? "No matching files"
                                  : "No uploads yet"}
                        </Heading>
                        <p>
                            {!loaded
                                ? "Check your API base in Condite settings and retry."
                                : "Upload files here, or attach them to a conversation."}
                        </p>
                    </div>
                )}
                {!!rows.length &&
                    (view === "list" ? (
                        <div className="condite-table-wrap">
                            <table className="condite-table">
                                <thead>
                                    <tr>
                                        <th>Name</th>
                                        <th>Size</th>
                                        <th>Status</th>
                                        <th>Created</th>
                                        <th>Actions</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {rows.map(row => (
                                        <tr key={row.id}>
                                            <td className="condite-file-name">
                                                <strong>{row.filename}</strong>
                                                <small>{row.mime_type}</small>
                                            </td>
                                            <td
                                                title={`${row.size_bytes} bytes`}
                                            >
                                                {bytes(row.size_bytes)}
                                            </td>
                                            <td>
                                                <span className="condite-status">
                                                    {row.status}
                                                </span>
                                            </td>
                                            <td>
                                                {new Date(
                                                    row.created_at
                                                ).toLocaleString()}
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
                                        {row.status}
                                    </span>
                                    <Heading tag="h3" className="condite-break">
                                        {row.filename}
                                    </Heading>
                                    <p>
                                        {bytes(row.size_bytes)} ·{" "}
                                        {row.mime_type}
                                    </p>
                                    <p>
                                        {new Date(
                                            row.created_at
                                        ).toLocaleString()}
                                    </p>
                                    {actions(row)}
                                </li>
                            ))}
                        </ul>
                    ))}
                <footer className="condite-toolbar">
                    <span className="condite-muted">
                        {rows.length} shown · Filters apply to this page
                    </span>
                    <Button
                        variant="secondary"
                        size="small"
                        disabled={busy || !history.length}
                        onClick={() =>
                            void navigate(history.at(-1) ?? null, true)
                        }
                    >
                        Previous
                    </Button>
                    <Button
                        variant="secondary"
                        size="small"
                        disabled={busy || !page.next_after}
                        onClick={() => void navigate(page.next_after)}
                    >
                        Next
                    </Button>
                </footer>
                <p className="condite-muted">
                    Download links currently expire. “Confirm” checks a pending
                    upload in storage; it does not re-upload missing bytes.
                </p>
            </div>
        </section>
    );
}
