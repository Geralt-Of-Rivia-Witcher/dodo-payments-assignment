FROM rust:1.83-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev openssl-dev pkgconfig

COPY Cargo.toml ./
COPY services/invoice-api/Cargo.toml services/invoice-api/Cargo.toml
COPY services/mock-psp/Cargo.toml services/mock-psp/Cargo.toml
COPY services/invoice-api/src services/invoice-api/src
COPY services/mock-psp/src services/mock-psp/src

RUN cargo build --release -p mock-psp

FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates
COPY --from=builder /app/target/release/mock-psp /app/mock-psp
EXPOSE 8081
CMD ["/app/mock-psp"]
