FROM clux/muslrust:stable as builder
ADD --chown=rust:rust . ./
RUN cargo build --release

FROM alpine:latest
COPY --from=builder /volume/target/x86_64-unknown-linux-musl/release/stablessh /usr/local/bin/
ENTRYPOINT [ "/usr/local/bin/stablessh" ]
