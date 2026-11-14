FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Sao chép giao diện Web
COPY ui /app/ui

# Tải binary đã được GitHub Actions build sẵn từ GitHub Releases (chỉ mất ~2s, không tốn RAM/CPU của Render)
RUN curl -fsSL -o /app/discord-reactor-server https://github.com/khanhdocnope/img/releases/latest/download/discord-reactor-server || \
    curl -fsSL -o /app/discord-reactor-server https://github.com/khanhdocnope/img/releases/download/v1.0.0/discord-reactor-server && \
    chmod +x /app/discord-reactor-server

ENV PORT=10000
EXPOSE 10000

RUN mkdir -p /app/data

CMD ["/app/discord-reactor-server"]
