# Docker ingress examples

Copy core's `.env.example` to `.env` and supply real storage credentials and a
random root credential of at least 32 characters. Configure `core.toml` and
`dashboard.toml` here: the core public API URL and dashboard origins must match
externally reachable HTTPS URLs. These TOML files mount read-only. Compose
`env_file` explicitly injects referenced core secrets into the container; Compose's
own `.env` interpolation alone does not do this. Never commit resolved credentials.

For Caddy, set your hostname in `Caddyfile` and both TOML files, point DNS at the
server, then run from the repository root:

```sh
docker compose -f docker/examples/compose.yaml -f docker/examples/compose.caddy.yaml up -d
```

Caddy terminates HTTPS and routes `/v1/*` to core and other paths to the dashboard.
Root and ordinary users log in with their own API keys; no bootstrap dashboard
key or separate administrator password is needed. Core enforces API access.
Public share paths must remain reachable without dashboard login. Preserve Host
and trusted forwarded headers, and avoid caching authenticated/API responses.

The isolated backend network connects dashboard, core and ingress; core also has
outbound access for S3. These production examples publish only ingress ports.
Do not combine them with `docker/compose.yaml`, which publishes development ports.
Signed PUT uploads go directly to private storage, requiring HTTPS and bucket
CORS for the dashboard origin. Downloads stream through core.

The tunnel example remains available for an existing Cloudflare Tunnel. Configure
its ingress to route `/v1/*` to core and dashboard pages to dashboard. If Access
also protects the dashboard, exclude public share/ticket routes from that extra
login. Keep tunnel credentials private. Back up SQL before upgrades and allow
legacy public-copy cleanup to finish; old storage links cannot be redirected.
