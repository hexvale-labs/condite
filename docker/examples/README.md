# Dashboard behind a proxy

Condite core and the dashboard currently serve **plain HTTP only**. Built-in TLS
is not currently planned, though it could be added eventually. For deployments
in 2026, put Condite behind cloudflared or a reverse proxy that handles HTTPS.
**Exposing the dashboard directly to the internet on port 80 is a horrible idea:**
traffic is unencrypted, and every visitor can use the dashboard's configured API
key. HTTPS alone does not restrict who can use that key; protect the dashboard
with authentication or trusted private access.

These examples publish no core or dashboard ports. Docker DNS connects the
proxy to `dashboard:4321`, and the dashboard's server-side API proxy connects to
`core:8080` on an `internal: true` network. Only core and the ingress container
also join an outbound-capable network. Do not combine these files with
[`../compose.yaml`](../compose.yaml), which publishes development ports.

The request path is browser → HTTPS ingress → HTTP dashboard → HTTP core.
File uploads/downloads still go directly between the browser and S3 using signed
URLs. Keep browser-facing storage URLs HTTPS and configure bucket CORS for the
public dashboard origin; the ingress does not rewrite those URLs. See the
[dashboard README](../../clients/dashboard/README.md).

## Prepare

Run commands from the repository root. Configure `crates/condite-core/.env` from
its example, including real storage settings. Choose a hostname you control:

```sh
export DASHBOARD_HOST=dashboard.example.com
export DASHBOARD_ORIGIN="https://$DASHBOARD_HOST"
# Check CPU/RAM/disk first; this builds the Rust SDKs as well as the dashboard.
docker build -f docker/dashboard.Dockerfile \
  --build-arg CARGO_BUILD_JOBS=1 \
  --build-arg DASHBOARD_ORIGIN="$DASHBOARD_ORIGIN" \
  -t condite-dashboard:local .

# Bootstrap core without publishing any ports.
docker compose -f docker/examples/compose.yaml run -d --name condite-bootstrap core
docker exec condite-bootstrap ctl apikey generate
# Save the generated key privately, then remove the bootstrap container.
docker stop condite-bootstrap
docker rm condite-bootstrap
read -rsp 'Dashboard API key: ' CONDITE_API_KEY; echo
export CONDITE_API_KEY
```

`DASHBOARD_ORIGIN` is a **build-time** host-validation setting. Setting it only at
runtime does not work; rebuild when changing the public origin. The published
image may have been built for a different origin, so these examples use a local
build. Keep keys and tunnel tokens out of Git. Back up the persistent core volume
before upgrading; startup applies database migrations.

## Caddy: HTTPS and password protection

Point the hostname's DNS at the server and allow inbound TCP 80/443 to **Caddy**.
Port 80 serves Caddy's HTTPS redirect/certificate handling, not the dashboard.
Generate a password hash interactively:

```sh
docker run --rm -it caddy:2 caddy hash-password
read -rp 'Caddy password hash: ' DASHBOARD_PASSWORD_HASH
export DASHBOARD_PASSWORD_HASH
docker compose -f docker/examples/compose.yaml \
  -f docker/examples/compose.caddy.yaml up -d
```

Open `https://dashboard.example.com/en/` and sign in as `admin` with the password
you hashed. [Caddy automatically obtains and renews certificates](https://caddyserver.com/docs/automatic-https).
Its [basic_auth](https://caddyserver.com/docs/caddyfile/directives/basic_auth)
protects SSR pages and API requests. The [reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy)
preserves the public host and sets forwarded headers so dashboard write-origin
checks see the HTTPS origin. Do not cache SSR or authenticated API responses.

For an existing Docker reverse proxy, attach it to this stack's backend network
(or use a dedicated shared ingress network), target `http://dashboard:4321`,
preserve the public Host, and overwrite forwarded host/protocol headers with
trusted values. Keep core private and enforce authentication at the proxy.
No Docker socket mount or automatic container discovery is needed here.

## Cloudflare Tunnel: no inbound ports

As an alternative to Caddy, create a remotely managed
[Cloudflare Tunnel](https://developers.cloudflare.com/tunnel/get-started/).
**Before publishing it**, configure a Cloudflare Access self-hosted application
for the dashboard hostname with an allow policy limited to your users. A tunnel
alone does not authenticate visitors.

Set the tunnel's published application hostname to `dashboard.example.com` and
its service URL to `http://dashboard:4321`. Preserve the public HTTP Host header
(do not override it with the Docker service name). Use the same public HTTPS
origin that was compiled into the dashboard.

```sh
read -rsp 'Tunnel token: ' TUNNEL_TOKEN; echo
export TUNNEL_TOKEN
docker compose -f docker/examples/compose.yaml \
  -f docker/examples/compose.cloudflared.yaml up -d
```

Open `https://dashboard.example.com/en/` through Access. This variant publishes
**no host ports**; cloudflared connects outbound to Cloudflare, which terminates
public HTTPS. HTTP stays on the private Docker hop to the dashboard. The tunnel
and Caddy variants are alternatives; run only one. Pin ingress images to tested
versions or digests for reproducible production deployments.
