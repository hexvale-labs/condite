# syntax=docker/dockerfile:1
FROM rust:1.99.1-trixie AS rust
FROM oven/bun:1.4.4 AS bun
FROM node:22.23.4-trixie AS tools
COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
ENV CARGO_BUILD_JOBS=2 PATH="/usr/local/cargo/bin:$PATH" RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo
COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun
RUN cargo install --locked cargo-chef --version 0.1.78 && rustup target add wasm32-unknown-unknown
WORKDIR /src
COPY Cargo.lock ./
COPY scripts/release/install-bindgen.sh /tmp/install-bindgen.sh
ARG TARGETARCH
RUN bash /tmp/install-bindgen.sh "$(awk '/^name = "wasm-bindgen"$/ {found=1; next} found && /^version =/ {gsub(/"/, "", $3); print $3; exit}' Cargo.lock)" "$TARGETARCH" /usr/local/bin

FROM tools AS planner
COPY Cargo.toml ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

FROM tools AS builder
ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS
COPY --from=planner /src/recipe.json ./recipe.json
RUN cargo chef cook --locked --profile sdk --recipe-path recipe.json -p condite-sdk-node
RUN cargo chef cook --locked --profile sdk --target wasm32-unknown-unknown --recipe-path recipe.json -p condite-sdk-wasm
COPY package.json bun.lock ./
COPY clients/dashboard/package.json clients/dashboard/package.json
COPY clients/vencord/package.json clients/vencord/package.json
COPY clients/vencord-extension/package.json clients/vencord-extension/package.json
COPY crates/condite-sdk-node/package.json crates/condite-sdk-node/package.json
COPY crates/condite-sdk-wasm/package.json crates/condite-sdk-wasm/package.json
RUN bun install --frozen-lockfile --ignore-scripts
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY versions.json ./
COPY scripts/sdk ./scripts/sdk
RUN bun run build:sdk:release
COPY clients/dashboard clients/dashboard
COPY clients/i18n clients/i18n
COPY clients/shared clients/shared
ARG DASHBOARD_ORIGIN=http://localhost:4321
ENV DASHBOARD_ORIGIN=$DASHBOARD_ORIGIN
RUN bun run build:dashboard
# Install only runtime dependencies into a separate tree. Bun workspace links
# stay valid because package directories and generated SDKs travel together.
RUN mkdir -p /runtime/clients/dashboard /runtime/crates/condite-sdk-node /runtime/crates/condite-sdk-wasm \
    && cp package.json bun.lock /runtime/ \
    && cp -a clients/dashboard/dist clients/dashboard/package.json /runtime/clients/dashboard/ \
    && mkdir -p /runtime/clients/vencord /runtime/clients/vencord-extension \
    && cp clients/vencord/package.json /runtime/clients/vencord/ \
    && cp clients/vencord-extension/package.json /runtime/clients/vencord-extension/ \
    && cp -a crates/condite-sdk-node/package.json crates/condite-sdk-node/dist /runtime/crates/condite-sdk-node/ \
    && cp -a crates/condite-sdk-wasm/package.json crates/condite-sdk-wasm/dist /runtime/crates/condite-sdk-wasm/ \
    && find /runtime -type d -name node_modules -prune -exec rm -rf '{}' + \
    && cd /runtime && bun install --production --frozen-lockfile \
    && find /runtime/clients/dashboard -mindepth 1 -maxdepth 1 ! -name dist ! -name node_modules ! -name package.json -exec rm -rf '{}' + \
    && rm -rf /runtime/clients/vencord /runtime/clients/vencord-extension /runtime/clients/i18n /runtime/clients/shared

FROM gcr.io/distroless/nodejs22-debian13:nonroot
ENV NODE_ENV=production HOST=0.0.0.0 PORT=4321
COPY --from=builder --chown=65532:65532 /runtime/ /app/
WORKDIR /app/clients/dashboard
EXPOSE 4321
CMD ["dist/server/entry.mjs"]
