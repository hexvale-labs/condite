// SPDX-License-Identifier: MIT
/** Create only from a browser mount; audio is initialized by user activation. */
export function createTickPlayer() {
  let context: AudioContext | undefined;
  let enabled = false;
  let disposed = false;
  return {
    async enable(): Promise<boolean> {
      if (disposed || typeof AudioContext === "undefined") return false;
      try {
        context ??= new AudioContext();
        await context.resume();
        if (disposed) return false;
        enabled = true;
        return true;
      } catch {
        enabled = false;
        return false;
      }
    },
    mute() {
      enabled = false;
    },
    tick() {
      if (!enabled || context?.state !== "running") return;
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.type = "sine";
      oscillator.frequency.setValueAtTime(900, context.currentTime);
      oscillator.frequency.exponentialRampToValueAtTime(
        160,
        context.currentTime + 0.035,
      );
      gain.gain.setValueAtTime(0.025, context.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, context.currentTime + 0.04);
      oscillator.connect(gain).connect(context.destination);
      oscillator.start();
      oscillator.stop(context.currentTime + 0.045);
      oscillator.onended = () => {
        oscillator.disconnect();
        gain.disconnect();
      };
    },
    dispose() {
      disposed = true;
      enabled = false;
      void context?.close().catch(() => {});
    },
  };
}
