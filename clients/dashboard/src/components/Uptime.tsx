import { createEffect, onCleanup } from "solid-js";
import { mountUptime } from "../../../shared/uptime-dom";
import {
  type HealthSample,
  formatUptime,
  sampleUptime,
} from "../../../shared/uptime";
import { messages, type Locale } from "../i18n";
import "../../../shared/uptime.css";
export default function Uptime(props: {
  health: HealthSample | null;
  locale: Locale;
}) {
  let element!: HTMLSpanElement;
  const initial = () =>
    props.health ? sampleUptime(props.health.runtime, "", 0, 0) : null;
  let controller: ReturnType<typeof mountUptime> | undefined;
  onCleanup(() => controller?.());
  createEffect(
    () => ({ health: props.health, locale: props.locale }),
    (value) => {
      if (controller)
        controller.update(value.health, value.locale, messages(value.locale));
      else
        controller = mountUptime(
          element,
          value.health,
          value.locale,
          messages(value.locale),
        );
    },
  );
  return (
    <span class="condite-uptime" ref={element}>
      {initial()
        ? formatUptime(initial()!.seconds, props.locale)
        : messages(props.locale).unknown}
    </span>
  );
}
