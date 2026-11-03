# syntax=docker/dockerfile:1
FROM rust:1.99.1-trixie AS chef
ENV CARGO_BUILD_JOBS=2
RUN cargo install --locked cargo-chef --version 0.1.78
WORKDIR /src
FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json
FROM chef AS builder
ARG CARGO_BUILD_JOBS=2
ENV CARGO_BUILD_JOBS=$CARGO_BUILD_JOBS
COPY --from=planner /src/recipe.json ./recipe.json
RUN cargo chef cook --locked --profile container --recipe-path recipe.json -p condite-core -p condite-ctl --bin condite-core --bin condite-ctl
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --locked --profile container -p condite-core -p condite-ctl --bin condite-core --bin condite-ctl \
    && mkdir -p /out/bin /out/data /out/run/condite \
    && cp target/container/condite-core target/container/condite-ctl /out/bin/ \
    && for name in ctl control condite condite-control; do ln -s condite-ctl /out/bin/$name; done \
    && ldd /out/bin/condite-core && ldd /out/bin/condite-ctl

# ldd confirms only libc/libm/libgcc are required; avoid C++ and OpenSSL runtimes.
RUN mkdir -p "/runtime-libs/$(gcc -print-multiarch)" \
    && cp "$(gcc -print-file-name=libgcc_s.so.1)" "/runtime-libs/$(gcc -print-multiarch)/"

FROM gcr.io/distroless/base-nossl-debian13:nonroot
COPY --from=builder /runtime-libs/ /usr/lib/
COPY --from=builder --chown=65532:65532 /out/ /app/
ENV PATH="/app/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin" \
    XDG_RUNTIME_DIR=/app/run
WORKDIR /app/data
EXPOSE 8080
ENTRYPOINT ["/app/bin/condite-core"]
