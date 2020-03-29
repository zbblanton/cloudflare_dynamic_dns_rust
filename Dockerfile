FROM rust:alpine AS builder

RUN apk update && apk add build-base openssl-dev

RUN git clone https://github.com/zbblanton/cloudflare_dynamic_dns_rust.git && \
    cd cloudflare_dynamic_dns_rust && \
    RUSTFLAGS="-C target-feature=-crt-static" cargo build

FROM alpine

COPY --from=builder cloudflare_dynamic_dns_rust/target/debug/cloudflare_dynamic_dns_rust /app/

CMD ["./app/cloudflare_dynamic_dns_rust"]