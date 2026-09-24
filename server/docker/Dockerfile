# syntax=docker/dockerfile:1
#
# SpiderNet node container image (multi-stage).
#
# NO WARRANTY — SpiderNet is provided "as is", without warranty of any kind
# (AGPL-3.0, sections 15/16). Running a node is entirely at the operator's
# own responsibility, including all traffic sent and received with it.
#
# Build context is the `server/` directory (see docker-compose.yml):
#   docker build -t spidernet-node -f docker/Dockerfile ..

# --- Stage 1: build the Rust workspace --------------------------------------

FROM rust:1-slim AS build

WORKDIR /server

COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# --locked: the build must match the committed Cargo.lock exactly.
RUN cargo build --release --locked --workspace

# --- Stage 2: runtime --------------------------------------------------------

# trixie, not bookworm: rust:1-slim is trixie-based, and binaries built
# against its glibc (2.41) need a matching runtime (bookworm only has 2.36).
FROM debian:trixie-slim

# Yggdrasil sidecar: current stable release of yggdrasil-go. Release v0.5.14
# ships official .deb packages (no raw tarballs). Binary inside: /usr/bin/yggdrasil.
# Digests from https://api.github.com/repos/yggdrasil-network/yggdrasil-go/releases/latest
ARG YGGDRASIL_VERSION=0.5.14
ARG TARGETARCH

# ca-certificates: TLS for sn-exit/sn-fetch (rustls) and tls:// peers.
# curl is only needed to fetch the yggdrasil .deb and is removed again.
# TARGETARCH is only set by BuildKit; fall back to the host arch otherwise
# (both use the same amd64/arm64 naming as the yggdrasil .deb assets).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && ARCH="${TARGETARCH:-$(dpkg --print-architecture)}" \
    && case "$ARCH" in \
        amd64) YGGDRASIL_SHA256=72745496b09baa1c3f5a6741e1257a704cfdb9d00cdf3ddbef025987b239699d ;; \
        arm64) YGGDRASIL_SHA256=ac8c9a9a76c54b878fd251aef5a2bbd20375c6109b28b32435683441462360c9 ;; \
        *) echo "unsupported architecture: $ARCH" >&2; exit 1 ;; \
       esac \
    && curl -fsSL -o /tmp/yggdrasil.deb \
        "https://github.com/yggdrasil-network/yggdrasil-go/releases/download/v${YGGDRASIL_VERSION}/yggdrasil-${YGGDRASIL_VERSION}-${ARCH}.deb" \
    && echo "${YGGDRASIL_SHA256}  /tmp/yggdrasil.deb" | sha256sum -c - \
    && dpkg-deb -x /tmp/yggdrasil.deb /tmp/yggdrasil-root \
    && cp /tmp/yggdrasil-root/usr/bin/yggdrasil /usr/local/bin/yggdrasil \
    && rm -rf /tmp/yggdrasil.deb /tmp/yggdrasil-root \
    && apt-get purge -y curl \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /server/target/release/sn-node /usr/local/bin/sn-node
COPY --from=build /server/target/release/sn-exit /usr/local/bin/sn-exit
COPY --from=build /server/target/release/sn-fetch /usr/local/bin/sn-fetch
COPY --from=build /server/target/release/sn-fair /usr/local/bin/sn-fair

# WORKDIR matches the state volume in docker-compose.yml; a config that
# leaves `state_dir` at its default then resolves to the volume as well.
WORKDIR /var/lib/spidernet

ENTRYPOINT ["sn-node", "daemon", "--config", "/etc/spidernet/node.toml"]
