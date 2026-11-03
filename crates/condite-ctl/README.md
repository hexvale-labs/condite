# condite-ctl

Local administration over Condite's Unix control socket. Build/install with
`cargo install --path crates/condite-ctl --locked` from the repository root.

```sh
condite-ctl status
condite-ctl apikey generate --max-per-upload-size=100mb --max-total-storage-size=1gb
condite-ctl apikey update <key-id> --max-total-storage-size=2GiB
condite-ctl apikey delete <key-id>
condite-ctl upload delete <upload-id>
```

Generation defaults to 100 MiB per upload and 1 GiB total. Save the secret when
shown; it cannot be retrieved later. Use key IDs for updates and revocation.
Deleting a key revokes access and retains its files. Previously issued S3 URLs
remain usable until they expire.

Sizes require a whole number and B, KB/KiB, MB/MiB, GB/GiB or TB/TiB (case
insensitive). All multipliers are binary: `100mb` equals `100MiB`. Bare numbers,
fractions, negative values and overflow are errors. Zero disables new bytes.

The CLI checks `$XDG_RUNTIME_DIR/condite/condite.sock`, falling back to
`/run/condite/condite.sock` when absent. Use `--socket <path>` to override,
`--json` for scripts, and `--timeout <seconds>` to adjust the default 10-second
socket timeout. Errors go to stderr and exit nonzero. `--help` works offline.

Use `just build ctl`, `just check ctl` (format, Clippy and existing CLI tests),
`just install ctl` (install on PATH), or `just run ctl` (temporary interactive
shell). `just run` includes this shell alongside the API and dashboard in mprocs.
Select **control** and press Ctrl-a to interact. Exiting the shell does not stop
other processes. All processes share the same development control socket.

For remote control, copy `ctl.example.toml` to `ctl.toml` and set `endpoint` and
`api_key = "${CONDITE_OPERATOR_KEY}"`. Pass `--config /path/ctl.toml` to select
another file. `.env` supplies referenced secrets only. HTTP requires a root key;
Unix discovery remains the default when no endpoint is configured. Remote HTTP
requires HTTPS, or explicit `allow_insecure_http = true` on a trusted Docker
network. Requests are bounded and time out. `apikey list` and `apikey inspect ID`
show identities, limits and usage without existing key credentials. Root
credentials are managed in core's TOML.
