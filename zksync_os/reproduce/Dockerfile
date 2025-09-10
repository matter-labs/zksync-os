# Use a specific version of Rust for reproducibility
# Based off zksync-airbender/tools/reproduce/Dockerfile
# Pinning os version and architecture for reproducibility
FROM --platform=linux/amd64 debian:bullseye-slim@sha256:4edd2740ebd41d71ef78eed0081c8979e287e2496c6fca892f5936d1e22adcd3 AS builder

# Freeze APT to a specific point in time (matches base image date)
ARG DEBIAN_SNAPSHOT=20250811T000000Z

# Use snapshot.debian.org and disable Valid-Until
RUN set -eux; \
  printf 'Acquire::Check-Valid-Until "false";\n' > /etc/apt/apt.conf.d/99snapshot; \
  printf 'Acquire::http::No-Cache "true";\n'     > /etc/apt/apt.conf.d/99no-cache; \
  echo "deb [check-valid-until=no] http://snapshot.debian.org/archive/debian/${DEBIAN_SNAPSHOT} bullseye main"            >  /etc/apt/sources.list; \
  echo "deb [check-valid-until=no] http://snapshot.debian.org/archive/debian-security/${DEBIAN_SNAPSHOT} bullseye-security main" >> /etc/apt/sources.list; \
  apt-get -o Acquire::Check-Valid-Until=false update

# Setting a fixed SOURCE_DATE_EPOCH for reproducible builds
# reproduce.sh will set it based on last commit date
ARG SOURCE_DATE_EPOCH=1700000000
ENV SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH} \
    CARGO_INCREMENTAL=0 \
    TZ=UTC LANG=C.UTF-8 LC_ALL=C.UTF-8

# Toolchain (paths pinned so absolute paths don’t leak)
ENV CARGO_HOME=/opt/cargo RUSTUP_HOME=/opt/rustup
ENV PATH=/opt/cargo/bin:$PATH
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates curl git build-essential libssl-dev pkg-config \
 && rm -rf /var/lib/apt/lists/*
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain nightly-2025-05-24 --profile minimal
RUN rustup target add riscv32i-unknown-none-elf \
 && cargo install cargo-binutils --locked \
 && rustup component add llvm-tools-preview

ENV CARGO_TARGET_RISCV32I_UNKNOWN_NONE_ELF_AR=llvm-ar
ENV CARGO_TARGET_RISCV32I_UNKNOWN_NONE_ELF_RANLIB=llvm-ranlib


COPY . zksync_os

WORKDIR /zksync_os/zksync_os

RUN ./dump_bin.sh
RUN ./dump_bin.sh --type evm-replay
RUN ./dump_bin.sh --type evm-replay-logging-enabled
RUN ./dump_bin.sh --type server-logging-enabled
RUN ./dump_bin.sh --type server
RUN ./dump_bin.sh --type multiblock-batch
RUN ./dump_bin.sh --type multiblock-batch-logging-enabled
