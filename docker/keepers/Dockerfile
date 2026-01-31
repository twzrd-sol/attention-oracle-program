# ============================================================
# Stage 1: Install dependencies
# ============================================================
FROM node:24-bookworm-slim AS builder

RUN corepack enable && corepack prepare pnpm@latest --activate

WORKDIR /app

# Copy lockfile + manifests first for layer caching
COPY package.json pnpm-lock.yaml ./

# Install ALL deps — tsx and typescript are devDependencies but
# required at runtime (keepers run TypeScript source directly)
RUN pnpm install --frozen-lockfile

# ============================================================
# Stage 2: Runtime
# ============================================================
FROM node:24-bookworm-slim

# Doppler CLI for secrets injection at runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl gnupg ca-certificates \
    && curl -sLf --retry 3 --tlsv1.2 --proto "=https" \
       'https://packages.doppler.com/public/cli/gpg.DE2A7741A397C129.key' \
       | gpg --dearmor -o /usr/share/keyrings/doppler-archive-keyring.gpg \
    && echo "deb [signed-by=/usr/share/keyrings/doppler-archive-keyring.gpg] \
       https://packages.doppler.com/public/cli/deb/debian any-version main" \
       > /etc/apt/sources.list.d/doppler-cli.list \
    && apt-get update && apt-get install -y --no-install-recommends doppler \
    && apt-get purge -y gnupg \
    && apt-get autoremove -y \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN groupadd -r keeper && useradd -r -g keeper -m keeper

WORKDIR /app

# Copy installed node_modules from builder
COPY --from=builder /app/node_modules ./node_modules

# Copy project files needed at runtime
COPY package.json tsconfig.json ./
COPY scripts/ ./scripts/

# Copy entrypoint and healthcheck
COPY docker/keepers/entrypoint.sh /usr/local/bin/entrypoint.sh
COPY docker/keepers/healthcheck.sh /usr/local/bin/healthcheck.sh
RUN chmod +x /usr/local/bin/entrypoint.sh /usr/local/bin/healthcheck.sh

# Writable /tmp for heartbeat file
RUN mkdir -p /tmp && chown keeper:keeper /tmp

USER keeper

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]

# Default — overridden per service in compose
CMD ["npx", "tsx", "scripts/keepers/compound-keeper.ts"]
