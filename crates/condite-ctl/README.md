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
