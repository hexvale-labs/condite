/*
 * Vencord, a Discord client mod
 * Copyright (c) 2026 Condite contributors
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

import "./shared/uptime.css";

import { React, useEffect, useRef } from "@webpack/common";

import { useTranslation } from "./i18n";
import type { HealthSample } from "./shared/uptime";
import { mountUptime } from "./shared/uptime-dom";
export function Uptime({ health }: { health: HealthSample | null }) {
    const element = useRef<HTMLSpanElement>(null);
    const { locale, t } = useTranslation();
    const controller = useRef<ReturnType<typeof mountUptime> | null>(null);
    useEffect(() => {
        if (element.current) controller.current = mountUptime(element.current, health, locale, t);
        return () => { controller.current?.(); controller.current = null; };
    }, []);
    useEffect(() => { controller.current?.update(health, locale, t); }, [health, locale]);
    return <span ref={element} className="condite-uptime" />;
}
