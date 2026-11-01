/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import ErrorBoundary from "@components/ErrorBoundary";
import {
    createRoot,
    React,
    ReactDOM,
    showToast,
    useEffect,
    useState
} from "@webpack/common";
import type { Root } from "react-dom/client";

import { FileManager } from "./fileManager";
import { t, useTranslation } from "./i18n";

let opened = false;
const subscribers = new Set<(value: boolean) => void>();
let root: Root | undefined;
let container: HTMLDivElement | undefined;
export function setPaneOpen(value: boolean) {
    opened = value;
    for (const subscriber of subscribers) subscriber(value);
}

function UploadsNavigation() {
    const { t, locale, dir } = useTranslation();
    const [open, setOpen] = useState(opened);
    useEffect(() => {
        subscribers.add(setOpen);
        return () => {
            subscribers.delete(setOpen);
        };
    }, []);
    return (
        <button
            lang={locale}
            dir={dir}
            className="condite-nav"
            aria-pressed={open}
            onClick={() => setPaneOpen(!open)}
        >
            <svg
                width="24"
                height="24"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.8"
                aria-hidden="true"
            >
                <path d="M12 16V3m-4 4 4-4 4 4M4 15v5h16v-5" />
            </svg>
            <span>{t.uploads}</span>
        </button>
    );
}

function UploadsPane() {
    const [open, setOpen] = useState(opened);
    const [host, setHost] = useState<HTMLElement | null>(null);
    useEffect(() => {
        subscribers.add(setOpen);
        return () => {
            subscribers.delete(setOpen);
        };
    }, []);
    useEffect(() => {
        if (!open) {
            setHost(null);
            return;
        }
        const locate = () => {
            // Discord's Home and channel views expose a main landmark. No hashed CSS class dependency.
            const main = document.querySelector<HTMLElement>("main");
            setHost(main);
            if (!main) {
                showToast({
                    text: t("failedLoad"),
                    variant: "critical"
                });
                setPaneOpen(false);
            }
        };
        locate();
        const observer = new MutationObserver(locate);
        observer.observe(document.body, { childList: true, subtree: true });
        const navigate = (event: MouseEvent) => {
            if (
                (event.target as Element)?.closest?.(
                    'a[href^="/channels/"], a[href="/shop"], a[href="/store"]'
                )
            )
                setPaneOpen(false);
        };
        const escape = (event: KeyboardEvent) => {
            if (
                event.key === "Escape" &&
                !document.querySelector('[role="dialog"]')
            )
                setPaneOpen(false);
        };
        document.addEventListener("click", navigate, true);
        document.addEventListener("keydown", escape);
        return () => {
            observer.disconnect();
            document.removeEventListener("click", navigate, true);
            document.removeEventListener("keydown", escape);
        };
    }, [open]);
    useEffect(() => {
        if (!host || !open) return;
        host.classList.add("condite-pane-host");
        return () => {
            host.classList.remove("condite-pane-host");
        };
    }, [host, open]);

    return open && host
        ? ReactDOM.createPortal(
              <div className="condite-page">
                  <ErrorBoundary>
                      <FileManager onClose={() => setPaneOpen(false)} />
                  </ErrorBoundary>
              </div>,
              host
          )
        : null;
}

export const renderUploadsNavigation = ErrorBoundary.wrap(UploadsNavigation, {
    noop: true
});

export function startNavigation() {
    container = document.createElement("div");
    document.body.append(container);
    root = createRoot(container);
    root.render(
        <ErrorBoundary>
            <UploadsPane />
        </ErrorBoundary>
    );
}

export function stopNavigation() {
    setPaneOpen(false);
    root?.unmount();
    root = undefined;
    container?.remove();
    container = undefined;
}
