import { createMemo, Show } from "solid-js";
import { messages, type Locale } from "../i18n";
import type { ExpiryChoice } from "../lib/expiry";

type Props = {
  locale: Locale;
  value: ExpiryChoice;
  onChange: (value: ExpiryChoice) => void;
  allowDefault?: boolean;
  allowFollow?: boolean;
  disabled?: boolean;
};
export default function ExpiryPicker(props: Props) {
  const t = createMemo(() => messages(props.locale));
  const update = (part: Partial<ExpiryChoice>) =>
    props.onChange({ ...props.value, ...part });
  return (
    <div class="expiry-picker">
      <label class="control-label">
        {t().expires}
        <select
          class="field w-full"
          value={props.value.mode}
          disabled={props.disabled}
          onChange={(e) =>
            update({ mode: e.currentTarget.value as ExpiryChoice["mode"] })
          }
        >
          <Show when={props.allowDefault}>
            <option value="default">{t().defaultRetention}</option>
          </Show>
          <Show when={props.allowFollow}>
            <option value="upload">{t().untilUploadExpires}</option>
          </Show>
          <option value="interval">{t().interval}</option>
          <option value="date">{t().customDate}</option>
          <option value="never">{t().never}</option>
        </select>
      </label>
      <Show when={props.value.mode === "date"}>
        <label class="control-label">
          {t().customDate} (UTC)
          <input
            class="field w-full"
            type="datetime-local"
            value={props.value.date}
            disabled={props.disabled}
            onInput={(e) => update({ date: e.currentTarget.value })}
          />
        </label>
      </Show>
      <Show when={props.value.mode === "interval"}>
        <div class="flex flex-wrap items-end gap-3">
          <label class="control-label flex-1">
            {t().interval}
            <select
              class="field w-full"
              value={props.value.subtract ? "subtract" : "add"}
              disabled={props.disabled}
              onChange={(e) =>
                update({ subtract: e.currentTarget.value === "subtract" })
              }
            >
              <option value="add">{t().addInterval}</option>
              <option value="subtract">{t().subtractInterval}</option>
            </select>
          </label>
          <label class="control-label flex-1">
            {t().days}
            <input
              class="field w-full"
              type="number"
              min="1"
              max="36500"
              step="1"
              value={props.value.days}
              disabled={props.disabled}
              onInput={(e) => update({ days: e.currentTarget.value })}
            />
          </label>
        </div>
        <div class="flex flex-wrap gap-2" role="group" aria-label={t().days}>
          {[1, 7, 14, 30].map((days) => (
            <button
              type="button"
              class="button button-small"
              disabled={props.disabled}
              aria-pressed={
                props.value.days === String(days) ? "true" : "false"
              }
              onClick={() => update({ days: String(days) })}
            >
              {days} {t().days}
            </button>
          ))}
        </div>
      </Show>
    </div>
  );
}
