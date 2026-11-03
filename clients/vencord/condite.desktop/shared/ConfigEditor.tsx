import { Button } from "@components/Button";
import { React, useState } from "@webpack/common";
import { stringify, type ClientConfig } from "./config";

export function ConfigEditor({
  current,
  importFile,
  apply,
}: {
  current: () => ClientConfig;
  importFile: () => Promise<ClientConfig | null>;
  apply: (config: ClientConfig) => void;
}) {
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  return (
    <div>
      <div className="condite-actions">
        <Button
          onClick={async () => {
            try {
              const config = await importFile();
              if (config) {
                apply(config);
                setText("");
                setError("");
              }
            } catch {
              setError(
                "Could not import TOML configuration. Check fields and environment references.",
              );
            }
          }}
        >
          Import TOML
        </Button>
        <Button onClick={() => setText(stringify(current()))}>
          Export TOML
        </Button>
      </div>
      {text && (
        <label>
          Configuration (contains your API key)
          <textarea
            value={text}
            readOnly
            rows={8}
            style={{ width: "100%" }}
            onFocus={(event) => event.currentTarget.select()}
          />
        </label>
      )}
      {error && <p role="alert">{error}</p>}
    </div>
  );
}
