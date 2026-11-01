/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import "./style.css";

import { ChatBarButton, type ChatBarButtonFactory } from "@api/ChatButtons";
import { definePluginSettings } from "@api/Settings";
import { Button } from "@components/Button";
import { FormSwitch } from "@components/FormSwitch";
import { Paragraph } from "@components/Paragraph";
import { copyToClipboard } from "@utils/clipboard";
import definePlugin, { type IconComponent, OptionType, type PluginNative } from "@utils/types";
import type { RenderModalProps } from "@vencord/discord-types";
import { ContextMenuApi, Menu, Modal, openModal, React, useState } from "@webpack/common";

import { ErrorMessage } from "./ErrorMessage";
import { UploadManager } from "./manager";
import { type PrivacyReport, retainedLabels } from "./workflow";

const Native = VencordNative.pluginHelpers.Condite as PluginNative<typeof import("./native")>;
const settings = definePluginSettings({
    apiKey: { type: OptionType.STRING, description: "Condite API key (saved locally in Vencord settings)", default: "" },
    retentionDays: { type: OptionType.NUMBER, description: "Default upload retention in days; 0 keeps files indefinitely", default: 14 },
    apiUrl: {
        type: OptionType.STRING,
        description: "Condite API base URL (HTTPS, or HTTP on localhost)",
        default: "http://127.0.0.1:8080"
    }
});

const ConditeIcon: IconComponent = ({ width = 24, height = 24, className }) => (
    <svg width={width} height={height} className={className} viewBox="0 0 24 24" fill="none" aria-hidden="true">
        <path d="M7 3h7l4 4v14H7V3Zm7 0v5h4M4 7v14" stroke="currentColor" strokeWidth="1.6" strokeLinejoin="round" />
        <path d="M10 13h5m-5 4h5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
    </svg>
);

type Uploaded = { filename: string; uri: string; expiresAt: string | null; report: PrivacyReport; };
let lastUpload: Uploaded | undefined;

function UploadResult({ props, file }: { props: RenderModalProps; file: Uploaded; }) {
    const [message, setMessage] = useState("");
    const [copyError, setCopyError] = useState<unknown>(null);
    const { report } = file;
    return (
        <Modal {...props} title="Condite upload ready" actions={[{ text: "Done", variant: "secondary", onClick: props.onClose }]}>
            <div className="vc-condite-form">
                <Paragraph className="vc-condite-filename">{file.filename}</Paragraph>
                <Paragraph>This link expires {file.expiresAt ?? "Never"}. Anyone with it can download the file until then.</Paragraph>
                <label className="vc-condite-field">Direct share link
                    <textarea readOnly value={file.uri} rows={3} onFocus={event => event.currentTarget.select()} />
                </label>
                <Button onClick={async () => {
                    try { await copyToClipboard(file.uri); setCopyError(null); setMessage("Direct share link copied."); }
                    catch (cause) { setCopyError(cause); setMessage("Select the link above and copy it manually."); }
                }}>Copy share link</Button>
                <Paragraph role="status">{message}</Paragraph>
                {!!copyError && <ErrorMessage error={copyError} />}
                <Paragraph>
                    Metadata cleaning: {report.metadataStripped ? "applied" : "off"}. Filename anonymization: {report.filenameAnonymized ? "applied" : "off"}. Encryption: {report.encrypted ? "applied" : "off"}.
                </Paragraph>
                {report.retained.length > 0 && <Paragraph>Retained content and limits: {retainedLabels(report.retained).join("; ")}. Identifying file content may remain.</Paragraph>}
                {report.encrypted && <Paragraph>Share the passphrase separately. Download the file and use Condite → Decrypt local file to save a decrypted copy.</Paragraph>}
            </div>
        </Modal>
    );
}

function OperationModal({ props, mode }: { props: RenderModalProps; mode: "upload" | "decrypt"; }) {
    const [retentionDays, setRetentionDays] = useState(settings.store.retentionDays);
    const [retentionDate, setRetentionDate] = useState("");
    const [useDate, setUseDate] = useState(false);
    const [stripMetadata, setStripMetadata] = useState(false);
    const [anonymizeFilename, setAnonymizeFilename] = useState(false);
    const [encrypt, setEncrypt] = useState(false);
    const [passphrase, setPassphrase] = useState("");
    const [confirmation, setConfirmation] = useState("");
    const [busy, setBusy] = useState(false);
    const [error, setError] = useState<unknown>(null);
    const [saved, setSaved] = useState(false);
    const needsPassphrase = mode === "decrypt" || encrypt;
    const clear = () => { setPassphrase(""); setConfirmation(""); };
    const close = () => { clear(); props.onClose(); };
    const start = async () => {
        if (busy) return;
        if (needsPassphrase && (!passphrase || (mode === "upload" && passphrase !== confirmation))) {
            setError(mode === "upload" ? "Enter a passphrase and matching confirmation." : "Enter the file's passphrase.");
            return;
        }
        setBusy(true);
        setError("");
        // Clear the fields as soon as the transient native operation begins.
        const secret = needsPassphrase ? passphrase : undefined;
        clear();
        try {
            if (mode === "upload") {
                if (!useDate && (!Number.isInteger(retentionDays) || retentionDays < 0 || retentionDays > 36500)) throw new Error("Choose a valid retention interval or Never.");
                const deadline = useDate ? new Date(retentionDate + "Z").getTime() : retentionDays === 0 ? null : Date.now() + retentionDays * 86400000;
                if (deadline !== null && (!Number.isFinite(deadline) || deadline <= Date.now())) throw new Error("Choose a future expiry date or Never.");
                const reply = await Native.chooseAndUpload(settings.store.apiUrl, { stripMetadata, anonymizeFilename, passphrase: secret }, settings.store.apiKey, { expires_at: deadline === null ? null : new Date(deadline).toISOString() });
                if (reply.status === "error") setError(reply);
                if (reply.status === "ok") {
                    lastUpload = reply;
                    props.onClose();
                    openModal(resultProps => <UploadResult props={resultProps} file={reply} />);
                }
            } else {
                const reply = await Native.chooseDecryptAndSave(secret!);
                if (reply.status === "error") setError(reply);
                if (reply.status === "ok") setSaved(true);
            }
        } catch (cause) {
            setError(cause);
        } finally { clear(); setBusy(false); }
    };
    return (
        <Modal {...props} onClose={close}
            title={mode === "upload" ? "Upload with Condite" : "Decrypt a local file"}
            actions={[
                { text: busy ? "Working…" : mode === "upload" ? "Choose file and upload" : "Choose encrypted file", variant: "primary", disabled: busy, loading: busy, onClick: start },
                { text: saved ? "Done" : "Close", variant: "secondary", disabled: busy, onClick: close }
            ]}
        >
            <div className="vc-condite-form">
                {mode === "upload" ? <>
                    <label className="vc-condite-field">Upload retention
                        <select disabled={busy} value={useDate ? "date" : String(retentionDays)} onChange={event => { const { value } = event.target; setUseDate(value === "date"); if (value !== "date") setRetentionDays(Number(value)); }}>
                            {[1, 7, 14, 30].map(days => <option key={days} value={days}>{days} days</option>)}
                            <option value="0">Never</option><option value="date">Choose date (UTC)</option>
                        </select>
                    </label>
                    {!useDate && retentionDays !== 0 && <label className="vc-condite-field">Custom interval (days)<input type="number" min="1" max="36500" disabled={busy} value={retentionDays} onChange={event => setRetentionDays(Number(event.target.value))} /></label>}
                    {useDate && <label className="vc-condite-field">Expiry date (UTC)<input type="datetime-local" disabled={busy} value={retentionDate} onChange={event => setRetentionDate(event.target.value)} /></label>}

                    <Paragraph className="vc-condite-endpoint">Upload destination: {settings.store.apiUrl}</Paragraph>
                    <FormSwitch title="Remove file metadata" value={stripMetadata} onChange={setStripMetadata} disabled={busy}
                        description="Clean supported metadata locally. If cleaning fails, the file is not uploaded. Identifying content may remain." />
                    <FormSwitch title="Anonymize filename" value={anonymizeFilename} onChange={setAnonymizeFilename} disabled={busy}
                        description="Replace the original name with a random name before upload." />
                    <FormSwitch title="Encrypt with a passphrase" value={encrypt} disabled={busy}
                        onChange={value => { setEncrypt(value); clear(); }}
                        description="Encrypt locally after metadata cleaning and filename changes." />
                </> : <Paragraph>Select a previously downloaded encrypted file. Decryption and saving stay on this device.</Paragraph>}
                {needsPassphrase && <>
                    <label className="vc-condite-field">Passphrase
                        <input type="password" autoComplete="off" maxLength={4096} value={passphrase} disabled={busy} onChange={event => setPassphrase(event.currentTarget.value)} />
                    </label>
                    {mode === "upload" && <label className="vc-condite-field">Confirm passphrase
                        <input type="password" autoComplete="off" maxLength={4096} value={confirmation} disabled={busy} onChange={event => setConfirmation(event.currentTarget.value)} />
                    </label>}
                    <Paragraph>The passphrase is used for this operation only. It is not saved in plugin settings.</Paragraph>
                </>}
                {mode === "upload" && <Paragraph>One file at a time, up to 300 MiB before processing and 100 MiB after processing. The share link is offered for copying; no Discord message is sent.</Paragraph>}
                {busy && <Paragraph role="status">Choose the file in the desktop dialog, then wait for processing to finish.</Paragraph>}
                {saved && <Paragraph role="status">Decrypted file saved.</Paragraph>}
                {!!error && <ErrorMessage error={error} />}
            </div>
        </Modal>
    );
}

const openOperation = (mode: "upload" | "decrypt") => openModal(props => <OperationModal props={props} mode={mode} />);

function ConditeMenu() {
    return (
        <Menu.Menu navId="vc-condite-menu" onClose={ContextMenuApi.closeContextMenu}>
            <Menu.MenuItem id="vc-condite" label="Condite">
                <Menu.MenuItem id="vc-condite-upload" label="Upload file with privacy options…" action={() => openOperation("upload")} />
                <Menu.MenuItem id="vc-condite-manager" label="Manage uploads and shared links…" action={() => openModal(props => <UploadManager props={props} apiUrl={settings.store.apiUrl} apiKey={settings.store.apiKey} />)} />
                <Menu.MenuItem id="vc-condite-decrypt" label="Decrypt local file…" action={() => openOperation("decrypt")} />
                <Menu.MenuItem id="vc-condite-result" label="Last share link…" disabled={!lastUpload} action={() => {
                    if (lastUpload) {
                        const file = lastUpload;
                        openModal(props => <UploadResult props={props} file={file} />);
                    }
                }} />
            </Menu.MenuItem>
        </Menu.Menu>
    );
}

const ComposerButton: ChatBarButtonFactory = ({ isAnyChat }) => isAnyChat ? (
    <ChatBarButton tooltip="Condite files" buttonProps={{ "aria-haspopup": "menu" }}
        onClick={event => ContextMenuApi.openContextMenu(event, ConditeMenu)}>
        <ConditeIcon />
    </ChatBarButton>
) : null;

export default definePlugin({
    name: "Condite",
    description: "Upload files to Condite with local privacy options, or decrypt a downloaded file. Desktop only.",
    authors: [{ name: "Condite contributors", id: 955090007335530506n }],
    settings,
    settingsAboutComponent: () => <div className="vc-condite-form"><Button onClick={() => openModal(props => <UploadManager props={props} apiUrl={settings.store.apiUrl} apiKey={settings.store.apiKey} />)}>Manage uploads</Button><a href="https://github.com/hexvale-labs/condite/blob/main/AI-NOTICE.md" target="_blank" rel="noreferrer">Written with AI assistance</a><Paragraph>API keys are saved locally in Vencord settings.</Paragraph></div>,
    chatBarButton: { icon: ConditeIcon, render: ComposerButton },
    stop() { lastUpload = undefined; }
});
