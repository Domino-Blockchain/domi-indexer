# use the official Bun image
# see all versions at https://hub.docker.com/r/oven/bun/tags
FROM oven/bun:1.1 as base

# Copy the lock and package file
COPY bun.lockb .
COPY package.json .
COPY tsconfig.json .

# Install dependencies
RUN bun install --frozen-lockfile

# Copy source code
COPY src ./src

# run the app
ENTRYPOINT ["bun", "src/index.ts"]
