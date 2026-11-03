/* SPDX-License-Identifier: GPL-3.0-or-later */
import { React, useEffect, useRef } from "@webpack/common";
import { mountUptime } from "./shared/uptime-dom";
import type { HealthSample } from "./shared/uptime";
import "./shared/uptime.css";
export function Uptime({ health }: { health: HealthSample | null }) {
    const element = useRef<HTMLSpanElement>(null);
    const controller = useRef<ReturnType<typeof mountUptime> | null>(null);
    const labels = {
        unknown: "Unknown", offline: "Offline", healthy: "Healthy", doomClock: "Time until doom (counting up)",
        enableTick: "Enable clock ticking", muteTick: "Mute clock ticking"
    };
    useEffect(() => {
        if (element.current) controller.current = mountUptime(element.current, health, navigator.language, labels);
        return () => { controller.current?.(); controller.current = null; };
    }, []);
    useEffect(() => { controller.current?.update(health, navigator.language, labels); }, [health]);
    return <span ref={element} className="condite-uptime" />;
}
