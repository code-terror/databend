# Build Stage
FROM ubuntu:22.04 as builder
## Install build dependencies.
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y openssl apt-transport-https ca-certificates pkg-config libssl-dev cmake clang curl git-all build-essential binutils-dev libunwind-dev libblocksruntime-dev liblzma-dev
RUN curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
RUN ${HOME}/.cargo/bin/rustup default nightly
RUN ${HOME}/.cargo/bin/cargo install honggfuzz
Add . /databend
WORKDIR /databend/tools/fuzz
RUN RUSTFLAGS="-Znew-llvm-pass-manager=no" HFUZZ_RUN_ARGS="--run_time $run_time --exit_upon_crash" ${HOME}/.cargo/bin/cargo hfuzz build

FROM ubuntu:22.04
RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y openssl pkg-config apt-transport-https ca-certificates
COPY --from=builder /databend/tools/fuzz/hfuzz_target/x86_64-unknown-linux-gnu/release/* /
