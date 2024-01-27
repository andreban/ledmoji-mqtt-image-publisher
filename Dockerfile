# The builder step builds the binary executable.
FROM rust:1 as builder
WORKDIR /app
COPY . .
RUN cargo install --bin daemon --path .

# The noto step downloads the noto emoji font from Github and decompresses it.
FROM debian:bookworm-slim as noto
RUN apt update
RUN apt install wget tar -y
RUN wget https://github.com/googlefonts/noto-emoji/archive/refs/tags/v2.042.tar.gz
RUN tar -xvf v2.042.tar.gz

# "runner" is the final image.
FROM debian:bookworm-slim as runner
RUN apt update
RUN apt install openssl ca-certificates -y

## Copy binary executable from builder stage.
COPY --from=builder /usr/local/cargo/bin/daemon /usr/local/bin/daemon

## Create /app directory and copy config files and assets.
WORKDIR /app
COPY --from=builder /app/assets /app/assets
COPY --from=noto /noto-emoji-2.042/png/128 /app/noto-emoji

## Set environment variables.
ENV EMOJI_DIRECTORY /app/noto-emoji
CMD ["daemon"]
