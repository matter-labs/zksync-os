# Use a specific version of Rust for reproducibility
# Based off zksync-airbender/tools/reproduce/Dockerfile
FROM debian:bullseye-slim AS builder

RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    git \
    libssl-dev \
    pkg-config

RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain nightly-2025-05-24
ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup target add riscv32i-unknown-none-elf
RUN cargo install cargo-binutils
RUN rustup component add llvm-tools-preview


COPY . zksync_os

WORKDIR zksync_os/zksync_os

RUN ./dump_bin.sh
RUN ./dump_bin.sh --type evm-replay
RUN ./dump_bin.sh --type server-logging-enabled
RUN ./dump_bin.sh --type server
RUN ./dump_bin.sh --type multiblock-batch
RUN ./dump_bin.sh --type multiblock-batch-logging-enabled
