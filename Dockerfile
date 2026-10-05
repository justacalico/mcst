# syntax=docker/dockerfile:1

# mcst all-in-one image: Flutter web bundle -> Rust binary -> slim runtime.
# The backend embeds frontend/dist/ at compile time via include_dir!.

FROM debian:trixie-slim AS frontend
ARG FLUTTER_VERSION=3.47.6
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        curl \
        git \
        unzip \
        xz-utils \
    && rm -rf /var/lib/apt/lists/* \
    && git clone --depth 1 --branch "$FLUTTER_VERSION" \
        https://github.com/flutter/flutter.git /opt/flutter
ENV PATH="/opt/flutter/bin:$PATH"
RUN flutter precache --web
WORKDIR /app
COPY flutter/pubspec.yaml flutter/pubspec.lock flutter/
RUN cd flutter && flutter pub get
COPY flutter/ flutter/
COPY scripts/build-flutter.sh scripts/build-flutter.sh
RUN bash scripts/build-flutter.sh

FROM rust:1.88-slim-trixie AS backend
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY migrations/ migrations/
COPY --from=frontend /app/frontend/dist/ frontend/dist/
RUN cargo build --release --locked

FROM debian:trixie-slim
# openjdk-21: runs modern Minecraft servers and Forge/NeoForge installers out
# of the box (mcst can also download additional runtimes itself).
# ca-certificates: HTTPS calls to Mojang/Paper/Modrinth/Adoptium APIs.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        openjdk-21-jre-headless \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home --shell /usr/sbin/nologin mcst
COPY --from=backend /app/target/release/mcst /usr/local/bin/mcst
USER mcst
WORKDIR /home/mcst
EXPOSE 25580
VOLUME ["/home/mcst/data"]
ENTRYPOINT ["mcst"]
CMD ["-d", "/home/mcst/data", "-p", "25580"]
