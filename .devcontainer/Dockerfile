FROM rust:bullseye

RUN apt-get update && apt-get install -y \
    sudo \
    software-properties-common \
    build-essential \
    gcc-aarch64-linux-gnu \
    gcc-arm-linux-gnueabihf \
    pkg-config \
    libssl-dev \
    binutils-aarch64-linux-gnu \
    graphviz

# Add to the rust components and cross-compilation target
RUN rustup component add clippy
RUN rustup component add rustfmt
RUN rustup target add aarch64-unknown-linux-gnu

# Install useful tools
RUN cargo install just cargo-modules loc


