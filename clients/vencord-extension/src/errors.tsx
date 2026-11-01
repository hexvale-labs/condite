/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import type { RenderModalProps } from "@vencord/discord-types";
import { Modal, openModal, React } from "@webpack/common";

import { useTranslation } from "./i18n";
import { presentError } from "./translations/errors";

export function ErrorMessage({ error }: { error: unknown }) {
    const { t } = useTranslation();
    const value = presentError(error, t);
    return <div role="alert" className="condite-error-details">
        <p>{value.message}</p>
        <details><summary>{t.rawError}</summary><pre>{value.raw}</pre></details>
    </div>;
}

export function showError(error: unknown, title: string) {
    openModal((props: RenderModalProps) => <ErrorModal props={props} error={error} title={title} />);
}
function ErrorModal({ props, error, title }: { props: RenderModalProps; error: unknown; title: string }) {
    const { t } = useTranslation();
    return <Modal {...props} title={title} actions={[{ text: t.close, variant: "secondary", onClick: props.onClose }]}>
        <div className="condite-modal-body"><ErrorMessage error={error} /></div>
    </Modal>;
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
