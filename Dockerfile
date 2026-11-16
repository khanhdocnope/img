FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Sao chép giao diện Web
COPY ui /app/ui

# Thêm biến build timestamp/commit để Docker trên Render KHÔNG BAO GIỜ cache lệnh tải binary
ARG BUILD_ID=20261116_boss_watcher_v1
RUN echo "Building container version: $BUILD_ID" && \
    curl -fsSL -H "Cache-Control: no-cache" -o /app/discord-reactor-server https://github.com/khanhdocnope/img/releases/download/v1.0.0/discord-reactor-server && \
    chmod +x /app/discord-reactor-server

ENV PORT=10000
EXPOSE 10000

RUN mkdir -p /app/data

CMD ["/app/discord-reactor-server"]
