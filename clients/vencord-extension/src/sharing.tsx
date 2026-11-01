/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { Button } from "@components/Button";
import type { RenderModalProps } from "@vencord/discord-types";
import { Modal, openModal, React, useEffect, useRef, useState } from "@webpack/common";

import { ErrorConfirmModal, ErrorMessage } from "./errors";
import { t as translate, useTranslation } from "./i18n";
import { createClient } from "./sdk";
import type { Share, Upload } from "./wire";

export type Expiry = "upload" | "never" | "date" | `${number}`;
export function expiryDate(choice: string, date: string, base = Date.now(), allowPast = false): string | null {
    if (choice === "never" || choice === "upload") return null;
    const value = choice === "date" ? new Date(`${date}Z`).getTime() : base + Number(choice) * 86400000;
    if (!Number.isFinite(value) || (!allowPast && value <= Date.now())) throw new Error(translate("invalidExpiry"));
    return new Date(value).toISOString();
}
export function ExpiryPicker({ value, onChange, date, onDate, share = false }: {
    value: string; onChange(value: Expiry): void; date: string; onDate(value: string): void; share?: boolean;
}) {
    const { t } = useTranslation();
    return <div className="condite-expiry-fields">
        <label>{share ? t.shareExpiry : t.retention}
            <select value={value} onChange={event => onChange(event.target.value as Expiry)}>
                {share && <option value="upload">{t.untilUploadExpires}</option>}
                {[1, 7, 14, 30].map(days => <option key={days} value={days}>{days} {t.days}</option>)}
                {![["upload", "never", "date"], ["1", "7", "14", "30"]].flat().includes(value) && <option value={value}>{value} {t.days}</option>}
                <option value="date">{t.customDate}</option><option value="never">{t.never}</option>
            </select>
        </label>
        {!Number.isNaN(Number(value)) && <label>{t.interval} ({t.days})<input type="number" min="1" max="36500" step="1" value={value} onChange={event => onChange(event.target.value as Expiry)} /></label>}
        {value === "date" && <label>{t.customDate} (UTC)<input type="datetime-local" value={date} onChange={event => onDate(event.target.value)} /></label>}
    </div>;
}
function SharingModal({ props, row, mode, changed }: { props: RenderModalProps; row: Upload; mode: "share" | "manage" | "expiry"; changed(): void }) {
    const { t, locale } = useTranslation();
    const [choice, setChoice] = useState<Expiry>(mode === "share" ? "upload" : "14");
    const [date, setDate] = useState("");
    const [shares, setShares] = useState<Share[]>([]);
    const [after, setAfter] = useState<string | null>(null);
    const [error, setError] = useState<unknown>(null);
    const [busy, setBusy] = useState(false);
    const [notice, setNotice] = useState("");
    const [interval, setInterval] = useState(7);
    const [deadline, setDeadline] = useState(row.expires_at);
    // A dialog is bound to the endpoint/key at opening, including subsequent pages.
    const [sdk] = useState(createClient);
    const locked = useRef(false);
    const run = async (operation: () => Promise<void>) => {
        if (locked.current) return;
        locked.current = true;
        setBusy(true); setError("");
        try { await operation(); } catch (cause) { setError(cause); } finally { locked.current = false; setBusy(false); }
    };
    const load = async (cursor: string | null = null) => {
        const page = await sdk.listShares(row.id, { limit: 20, after: cursor });
        setShares(previous => cursor ? [...previous, ...page.shares] : page.shares); setAfter(page.next_after);
    };
    useEffect(() => { if (mode === "manage") void run(() => load()); }, []);
    const save = (expires_at: string | null) => run(async () => {
        if (expires_at && new Date(expires_at).getTime() <= Date.now()) {
            openModal(confirmProps => <ErrorConfirmModal {...confirmProps} title={t.deleteTitle} confirmText={t.remove} cancelText={t.cancel}
                onConfirm={async () => { await sdk.delete(row.id); changed(); props.onClose(); }}>
                <p>{t.deletionWarning}</p><strong>{row.filename}</strong>
            </ErrorConfirmModal>);
            return;
        }
        const value = await sdk.updateRetention(row.id, { expires_at }); setDeadline(value.expires_at); setNotice(t.expirySaved); changed();
    });
    return <Modal {...props} title={mode === "share" ? t.share : mode === "manage" ? t.manageLinks : t.retention}
        actions={[{ text: t.close, variant: "secondary", onClick: props.onClose }]}>
        <div className="condite-manager condite-modal-body">
            <strong className="condite-break">{row.filename}</strong>
            {mode !== "manage" && <>
                <ExpiryPicker value={choice} onChange={setChoice} date={date} onDate={setDate} share={mode === "share"} />
                <Button disabled={busy} onClick={() => {
                    if (mode === "expiry") { try { void save(expiryDate(choice, date, Date.now(), true)); } catch (cause) { setError(cause); } }
                    else void run(async () => { const share = await sdk.createShare(row.id, { expires_at: expiryDate(choice, date), follows_upload: choice === "upload" }); setShares([share]); setNotice(t.shareCreated); changed(); });
                }}>{mode === "share" ? t.createShare : t.saveExpiry}</Button>
                {mode === "expiry" && <>
                    <p>{t.expires}: {deadline ? new Date(deadline).toLocaleString(locale, { timeZone: "UTC" }) + " UTC" : t.never}</p>
                    <label>{t.days}<input type="number" min="1" max="36500" value={interval} onChange={event => setInterval(Number(event.target.value))} /></label>
                    <div className="condite-actions">{[-1, 1].map(sign => <Button key={sign} disabled={busy || !Number.isInteger(interval) || interval < 1} variant="secondary" onClick={() => void save(new Date(new Date(deadline ?? Date.now()).getTime() + sign * interval * 86400000).toISOString())}>{sign > 0 ? t.addInterval : t.subtractInterval}</Button>)}</div>
                </>}
            </>}
            {shares.map(share => <section key={share.id} className="condite-share-row">
                <div className="condite-actions"><strong>{({ Creating: t.creating, Active: t.active, Revoking: t.revoking, Revoked: t.revoked, Expired: t.expired })[share.status]}</strong><span className="condite-muted">{new Date(share.created_at).toLocaleString(locale, { timeZone: "UTC" })} UTC</span></div>
                <p>{t.expires}: {share.expires_at ? new Date(share.expires_at).toLocaleString(locale, { timeZone: "UTC" }) + " UTC" : t.never}</p>
                <input aria-label={t.link} readOnly value={share.url} onFocus={event => event.target.select()} />
                <div className="condite-actions"><Button size="small" disabled={share.status !== "Active"} onClick={() => { void navigator.clipboard.writeText(share.url).then(() => { setError(null); setNotice(t.copied); }, cause => { setError(cause); setNotice(t.copyFailed); }); }}>{t.copy}</Button>
                    <Button size="small" variant="dangerSecondary" disabled={busy || ["Revoked", "Expired"].includes(share.status)} onClick={() => void run(async () => { await sdk.revokeShare(row.id, share.id); await load(); changed(); })}>{t.revoke}</Button></div>
            </section>)}
            {mode === "manage" && !busy && !shares.length && <p>{t.noShares}</p>}
            {after && <Button disabled={busy} onClick={() => void run(() => load(after))}>{t.next}</Button>}
            <p className="condite-muted">{mode === "expiry" ? t.cleanupHint : t.shareHint}</p>
            {notice && <p role="status">{notice}</p>}{!!error && <ErrorMessage error={error} />}
        </div>
    </Modal>;
}
export function openSharing(row: Upload, mode: "share" | "manage" | "expiry", changed: () => void) {
    openModal(props => <SharingModal props={props} row={row} mode={mode} changed={changed} />);
}
