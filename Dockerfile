# Build check for notes-manager (the GUI itself needs a display to run)

FROM rust:1-bookworm

RUN apt-get update && apt-get install -y \
    build-essential \
    pkg-config \
    libgtk-4-dev \
    libadwaita-1-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

WORKDIR /app/src-rust
RUN cargo test --release && cargo build --release
