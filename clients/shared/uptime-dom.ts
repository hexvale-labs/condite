// SPDX-License-Identifier: MIT
import {
  elapsedSeconds,
  formatUptime,
  sampleUptime,
  type HealthSample,
} from "./uptime";
import { createTickPlayer } from "./tick";
export type UptimeLabels = {
  unknown: string;
  offline: string;
  healthy: string;
  doomClock: string;
  enableTick: string;
  muteTick: string;
};
/** Owns the timer, visibility listener and audio for a single mounted display. */
export function mountUptime(
  element: HTMLElement,
  health: HealthSample | null,
  locale: string,
  labels: UptimeLabels,
) {
  let sample = health
    ? sampleUptime(
        health.runtime,
        health.started_at,
        Date.now(),
        performance.now(),
      )
    : null;
  const player = createTickPlayer();
  const dot = document.createElement("span");
  dot.className = "condite-availability";
  dot.setAttribute("role", "img");
  dot.dataset.healthy = String(!!health?.healthy);
  dot.setAttribute(
    "aria-label",
    health?.healthy ? labels.healthy : labels.offline,
  );
  const digits = document.createElement("span");
  digits.className = "condite-uptime-digits";
  const title = document.createElement("span");
  title.textContent = labels.doomClock;
  title.className = "condite-uptime-caption";
  const button = document.createElement("button");
  button.type = "button";
  button.className = "condite-tick-button";
  let muted = true,
    disposed = false,
    activation = 0;
  function syncButton() {
    button.textContent = muted ? "🔇" : "🔊";
    button.setAttribute(
      "aria-label",
      muted ? labels.enableTick : labels.muteTick,
    );
    button.title = muted ? labels.enableTick : labels.muteTick;
    button.setAttribute("aria-pressed", String(!muted));
    button.disabled = !health?.healthy || !sample;
  }
  button.onclick = async () => {
    if (muted) {
      button.disabled = true;
      const token = ++activation;
      const enabled = await player.enable();
      if (disposed || document.hidden || token !== activation) {
        player.mute();
        return;
      }
      muted = !enabled;
    } else {
      muted = true;
      player.mute();
    }
    if (!disposed) syncButton();
  };
  element.replaceChildren(dot, title, digits, button);
  let previous: string | null = null;
  function render() {
    const seconds = elapsedSeconds(sample, performance.now());
    const value =
      seconds === null ? labels.unknown : formatUptime(seconds, locale);
    if (digits.textContent !== value) {
      digits.textContent = value;
      if (
        previous !== null &&
        !matchMedia("(prefers-reduced-motion: reduce)").matches
      ) {
        digits.getAnimations().forEach((animation) => animation.cancel());
        digits.animate(
          [
            { transform: "translateY(-15%)", opacity: 0.5 },
            { transform: "translateY(0)", opacity: 1 },
          ],
          { duration: 180, easing: "cubic-bezier(0.23,1,0.32,1)" },
        );
      }
      previous = value;
      if (health?.healthy && !document.hidden) player.tick();
    }
  }
  let timer: ReturnType<typeof setInterval> | undefined;
  function visibility() {
    clearInterval(timer);
    timer = undefined;
    if (document.hidden) {
      activation++;
      muted = true;
      player.mute();
      syncButton();
    } else {
      render();
      if (health?.healthy && sample) timer = setInterval(render, 1000);
    }
  }
  syncButton();
  render();
  visibility();
  document.addEventListener("visibilitychange", visibility);
  const dispose = () => {
    activation++;
    disposed = true;
    clearInterval(timer);
    document.removeEventListener("visibilitychange", visibility);
    player.dispose();
    digits.getAnimations().forEach((animation) => animation.cancel());
    button.onclick = null;
  };
  dispose.update = (
    next: HealthSample | null,
    nextLocale: string,
    nextLabels: UptimeLabels,
  ) => {
    health = next;
    locale = nextLocale;
    labels = nextLabels;
    sample = health
      ? sampleUptime(
          health.runtime,
          health.started_at,
          Date.now(),
          performance.now(),
        )
      : null;
    dot.dataset.healthy = String(!!health?.healthy);
    dot.setAttribute(
      "aria-label",
      health?.healthy ? labels.healthy : labels.offline,
    );
    title.textContent = labels.doomClock;
    if (!health?.healthy || !sample) {
      activation++;
      muted = true;
      player.mute();
    }
    syncButton();
    visibility();
  };
  return dispose;
}
