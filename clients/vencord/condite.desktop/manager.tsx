/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { Button } from "@components/Button";
import { copyToClipboard } from "@utils/clipboard";
import type { PluginNative } from "@utils/types";
import type { RenderModalProps } from "@vencord/discord-types";
import { ConfirmModal, Modal, openModal, React, useEffect, useRef, useState } from "@webpack/common";

import type { Share, Upload } from "./wire";

const Native = VencordNative.pluginHelpers.Condite as PluginNative<typeof import("./native")>;
type Row = Omit<Upload, "size_bytes"> & { size_bytes: string };
type Operation = Parameters<typeof Native.manage>[2];
const formatDate = (value: string | null) => value ? new Date(value).toISOString().replace("T", " ") : "Never";
const safeLink = (value: string) => { const url = new URL(value); if (!["http:", "https:"].includes(url.protocol) || url.username || url.password) throw new Error("Unsupported download URL."); return url.href; };

export function UploadManager({ props, apiUrl, apiKey }: { props: RenderModalProps; apiUrl: string; apiKey: string }) {
    const [rows, setRows] = useState<Row[]>([]);
    const [next, setNext] = useState<string | null>(null);
    const [cursor, setCursor] = useState<string | null>(null);
    const [history, setHistory] = useState<(string | null)[]>([]);
    const [selected, setSelected] = useState<Row>();
    const [shares, setShares] = useState<Share[]>([]);
    const [shareNext, setShareNext] = useState<string | null>(null);
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState("");
    const [notice, setNotice] = useState("");
    const [mode, setMode] = useState("upload");
    const [date, setDate] = useState("");
    const [days, setDays] = useState(7);
    const [link, setLink] = useState("");
    const [deleting, setDeleting] = useState<string>();
    const call = async (operation: Operation, id = "", request: unknown = {}) => {
        const reply = await Native.manage(apiUrl, apiKey, operation, id, request);
        if (reply.status !== "ok") throw new Error(reply.message);
        return reply.value;
    };
    const locked = useRef(false);
    const run = async (operation: () => Promise<void>) => { if (locked.current) return; locked.current = true; setBusy(true); setError(""); try { await operation(); } catch (cause) { setError((cause as Error).message); } finally { locked.current = false; setBusy(false); } };
    const load = async (after: string | null = cursor) => { const page = await call("listUploads", "", { limit: 20, after }); setRows(page.uploads); setNext(page.next_after); };
    const loadShares = async (row: Row, after: string | null = null) => { const page = await call("listShares", row.id, { limit: 20, after }); setSelected(row); setShares(previous => after ? [...previous, ...page.shares] : page.shares); setShareNext(page.next_after); };
    useEffect(() => { void run(() => load()); }, []);
    const expiry = (allowPast = false) => {
        if (mode === "never" || mode === "upload") return null;
        const time = mode === "date" ? new Date(date + "Z").getTime() : Date.now() + Number(mode) * 86400000;
        if (!Number.isFinite(time) || (!allowPast && time <= Date.now())) throw new Error("Choose a future expiry date.");
        return new Date(time).toISOString();
    };
    const retain = async (expires_at: string | null) => {
        if (!selected) return;
        if (expires_at && new Date(expires_at).getTime() <= Date.now()) {
            const { id } = selected;
            openModal(confirmProps => <ConfirmModal {...confirmProps} title="Delete expired file?" confirmText="Delete" cancelText="Cancel" onConfirm={async setModalError => {
                try { await call("delete", id); setSelected(undefined); await load(); setNotice("File and its shared copies deleted."); }
                catch (cause) { setModalError((cause as Error).message); throw cause; }
            }}><p>This date has passed. Delete the file and all its shared copies?</p></ConfirmModal>);
            return;
        }
        const row = await call("updateRetention", selected.id, { expires_at }); setSelected(row); await load(); setNotice("Expiry saved.");
    };
    return <Modal {...props} title="Condite uploads" actions={[{ text: "Close", variant: "secondary", onClick: props.onClose }]}>
        <div className="vc-condite-manager">
            <div className="vc-condite-actions"><Button disabled={busy} onClick={() => void run(() => load())}>Refresh</Button><Button disabled={busy} variant="secondary" onClick={() => void run(async () => { const health = await call("health"); setNotice(health.healthy ? `Connected · ${health.runtime}` : "Unhealthy"); })}>Connection</Button></div>
            {error && <p role="alert" className="vc-condite-error">{error}</p>}
            {notice && <p role="status">{notice}</p>}
            {rows.map(row => <section key={row.id} className="vc-condite-record">
                <strong>{row.filename}</strong><small>{row.size_bytes} bytes · {row.visibility} · {row.status} · Expires {formatDate(row.expires_at)}{row.deletion_pending ? " · Cleanup pending" : ""}</small>
                <div className="vc-condite-actions">
                    {row.status === "Pending" ? <><Button size="small" disabled={busy} onClick={() => void run(async () => { await call("confirm", row.id); await load(); })}>Confirm</Button><Button size="small" disabled={busy || row.deletion_pending} onClick={() => void run(() => loadShares(row))}>Expiry</Button></> : <>
                        <Button size="small" disabled={busy || row.deletion_pending} onClick={() => void run(() => loadShares(row))}>Share / Manage links / Expiry</Button>
                        <Button size="small" disabled={busy} variant="secondary" onClick={() => void run(async () => { const value = await call("download", row.id); setLink(safeLink(value.uri)); setNotice(`Owner download expires ${formatDate(value.expires_at)}`); })}>Download</Button>
                    </>}
                    <Button size="small" variant="dangerSecondary" disabled={busy} onClick={() => setDeleting(row.id)}>Delete</Button>
                    {deleting === row.id && <><span>Delete this file and all shares?</span><Button size="small" variant="dangerPrimary" disabled={busy} onClick={() => void run(async () => { await call("delete", row.id); setDeleting(undefined); if (selected?.id === row.id) setSelected(undefined); await load(); })}>Confirm delete</Button><Button size="small" variant="secondary" onClick={() => setDeleting(undefined)}>Cancel</Button></>}
                </div>
            </section>)}
            {!rows.length && !busy && !error && <p>No uploads yet.</p>}
            <div className="vc-condite-actions"><Button disabled={busy || !history.length} variant="secondary" onClick={() => void run(async () => { const previous = history.at(-1) ?? null; await load(previous); setCursor(previous); setHistory(history.slice(0, -1)); })}>Previous page</Button><Button disabled={busy || !next} variant="secondary" onClick={() => void run(async () => { await load(next); setHistory([...history, cursor]); setCursor(next); })}>Next page</Button></div>
            {selected && <section className="vc-condite-record">
                <strong>{selected.filename}</strong>
                <label className="vc-condite-field">Expiry
                    <select value={mode} onChange={event => setMode(event.target.value)}><option value="upload">Until file expires</option>{[1,7,14,30].map(value => <option key={value} value={value}>{value} days</option>)}<option value="date">Choose date (UTC)</option><option value="never">Never</option></select>
                </label>
                {mode === "date" && <input aria-label="Expiry date UTC" type="datetime-local" value={date} onChange={event => setDate(event.target.value)} />}
                <div className="vc-condite-actions"><Button disabled={busy || selected.deletion_pending || selected.status !== "Confirmed"} onClick={() => void run(async () => { const share = await call("createShare", selected.id, { follows_upload: mode === "upload", expires_at: expiry() }); setLink(safeLink(share.url)); await loadShares(selected); await load(); })}>Share</Button><Button disabled={busy || mode === "upload" || selected.deletion_pending} variant="secondary" onClick={() => void run(() => retain(expiry(true)))}>Save file expiry</Button></div>
                <label className="vc-condite-field">Adjust file retention (days)<input type="number" min="1" max="36500" value={days} onChange={event => setDays(Number(event.target.value))} /></label>
                <div className="vc-condite-actions">{[-1,1].map(sign => <Button key={sign} variant="secondary" disabled={busy || !Number.isInteger(days) || days < 1} onClick={() => void run(() => retain(new Date(new Date(selected.expires_at ?? Date.now()).getTime() + sign * days * 86400000).toISOString()))}>{sign > 0 ? "Add interval" : "Subtract interval"}</Button>)}</div>
                <p>File expires {formatDate(selected.expires_at)}. A share cannot keep an expired file alive.</p>
                {shares.map(share => <section key={share.id} className="vc-condite-record"><strong>{share.status}</strong><small>Created {formatDate(share.created_at)} · Expires {formatDate(share.expires_at)}</small><textarea aria-label="Share URL" readOnly rows={2} value={share.url} onFocus={event => event.target.select()} /><div className="vc-condite-actions"><Button size="small" disabled={share.status !== "Active"} onClick={() => { void copyToClipboard(share.url).then(() => setNotice("Link copied."), () => setNotice("Select the link and copy it manually.")); }}>Copy</Button><Button size="small" variant="dangerSecondary" disabled={busy || ["Revoked","Expired"].includes(share.status)} onClick={() => void run(async () => { await call("revokeShare", selected.id, share.id); await loadShares(selected); await load(); })}>Revoke</Button></div></section>)}
                {!shares.length && <p>No shared links.</p>}{shareNext && <Button disabled={busy} onClick={() => void run(() => loadShares(selected, shareNext))}>More links</Button>}
            </section>}
            {link && <label className="vc-condite-field">Download / share link<textarea readOnly rows={2} value={link} onFocus={event => event.target.select()} /><Button onClick={() => { void copyToClipboard(link).then(() => setNotice("Link copied."), () => setNotice("Select the link and copy it manually.")); }}>Copy link</Button><a href={link} target="_blank" rel="noreferrer">Open download</a></label>}
            <p className="vc-condite-muted">Links go directly to storage. Condite removes expired objects approximately once per minute; downtime or storage failures can delay removal. Downloaded and cached copies cannot be recalled.</p>
        </div>
    </Modal>;
}
