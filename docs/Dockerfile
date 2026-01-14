# Stage 1: Build the Docusaurus site
FROM node:22-alpine AS builder

WORKDIR /app

# Copy package files
COPY docs/package*.json ./

# Install dependencies
RUN npm ci

# Copy docs source files
COPY docs/ ./

# Copy OpenAPI spec (referenced by docusaurus config as ../web/openapi.json)
COPY web/openapi.json /web/openapi.json

# Build the static site
RUN npm run build

# Stage 2: Serve with nginx
FROM nginx:alpine

# Copy built static files from builder
COPY --from=builder /app/build /usr/share/nginx/html

EXPOSE 80

CMD ["nginx", "-g", "daemon off;"]
