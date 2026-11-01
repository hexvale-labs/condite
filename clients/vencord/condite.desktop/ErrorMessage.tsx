/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import { Modal, React } from "@webpack/common";

import { presentError } from "./errors";

export function ErrorMessage({ error }: { error: unknown }) {
    const value = presentError(error);
    return <div role="alert" className="vc-condite-error">
        <p>{value.message}</p>
        <details><summary>Raw error</summary><pre>{value.raw}</pre></details>
    </div>;
}

export function ErrorConfirmModal(props: import("@vencord/discord-types").ConfirmModalProps) {
    const [error, setError] = React.useState<unknown>(null);
    const [busy, setBusy] = React.useState(false);
    const locked = React.useRef(false);
    return <Modal {...props} actions={[
        { text: props.confirmText, variant: "critical-primary", disabled: busy, loading: busy, onClick: async () => {
            if (locked.current) return;
            locked.current = true;
            setBusy(true); setError(null);
            try { await props.onConfirm?.(() => {}); props.onClose(); }
            catch (cause) { setError(cause); }
            finally { locked.current = false; setBusy(false); }
        } },
        { text: props.cancelText ?? "Cancel", variant: "secondary", disabled: busy, onClick: props.onClose }
    ]}>
        {props.children}
        {!!error && <ErrorMessage error={error} />}
    </Modal>;
}
