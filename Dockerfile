FROM --platform=$BUILDPLATFORM rust:1-slim as builder
ARG TARGETARCH
ARG LIBC=musl

ADD ./scripts/ /scripts
RUN NOSUDO=1 /scripts/setup-depends.sh

ADD . /rust
WORKDIR /rust
RUN TARGET=$(/scripts/resolve-arch.sh target) && \
  rustup target add $TARGET && \
  cargo build --release --target=$TARGET && \
  mv target/$TARGET/release/stablessh ./

FROM alpine:latest
COPY --from=builder /rust/stablessh /usr/local/bin/
ENTRYPOINT [ "/usr/local/bin/stablessh" ]
