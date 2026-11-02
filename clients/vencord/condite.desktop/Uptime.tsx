/* SPDX-License-Identifier: GPL-3.0-or-later */
import { React, useEffect, useRef } from "@webpack/common";
import { mountUptime } from "./shared/uptime-dom";
import type { HealthSample } from "./shared/uptime";
import "./shared/uptime.css";
export function Uptime({ health }: { health: HealthSample | null }) {
    const element = useRef<HTMLSpanElement>(null);
    useEffect(() => {
        if (element.current) return mountUptime(element.current, health, navigator.language, {
            unknown: "Unknown", offline: "Offline", healthy: "Healthy", doomClock: "Time until doom (counting up)",
            enableTick: "Enable clock ticking", muteTick: "Mute clock ticking"
        });
    }, [health]);
    return <span ref={element} className="condite-uptime" />;
}
