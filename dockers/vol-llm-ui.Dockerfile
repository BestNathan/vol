# vol-llm-ui Dockerfile (React SPA → nginx)
# =============================================================================
# Multi-stage build for the web frontend.
#
# Build:
#   docker build -t vol-llm-ui:latest -f dockers/vol-llm-ui.Dockerfile .
# =============================================================================

# ── Builder: Node.js + Vite build ────────────────────────────────────────────
FROM node:20-alpine AS builder

WORKDIR /app

# Install dependencies (cached layer)
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci

# Copy source and build
COPY frontend/ ./
RUN npm run build

# ── Runtime: nginx + static files ────────────────────────────────────────────
FROM nginx:1.27-alpine

# Copy nginx config
COPY dockers/nginx-frontend.conf /etc/nginx/conf.d/default.conf

# Copy Vite build output
COPY --from=builder /app/dist/ /usr/share/nginx/html/

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
