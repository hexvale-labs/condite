import { createMemo, Show } from "solid-js";
import { presentError } from "../../../i18n/errors";
import { messages, type Locale } from "../i18n";

export default function ErrorMessage(props: {
  error: unknown;
  locale: Locale;
}) {
  const t = createMemo(() => messages(props.locale));
  const formatted = createMemo(() => presentError(props.error, t()));
  return (
    <Show when={!!props.error}>
      <div class="error-message text-sm text-red-800" role="alert">
        <p>{formatted().message}</p>
        <details>
          <summary>{t().rawError}</summary>
          <pre>{formatted().raw}</pre>
        </details>
      </div>
    </Show>
  );
}
