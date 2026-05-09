FROM rust:1.88-alpine AS builder
WORKDIR /app
RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

COPY Cargo.toml ./
COPY services/invoice-api/Cargo.toml services/invoice-api/Cargo.toml
COPY services/mock-psp/Cargo.toml services/mock-psp/Cargo.toml
COPY migrations migrations
COPY services/invoice-api/src services/invoice-api/src
COPY services/mock-psp/src services/mock-psp/src

RUN cargo build --release -p invoice-api

FROM alpine:3.20
WORKDIR /app
RUN apk add --no-cache ca-certificates postgresql-client
COPY --from=builder /app/target/release/invoice-api /app/invoice-api
COPY migrations /app/migrations
COPY scripts/wait-and-migrate.sh /app/wait-and-migrate.sh
ENV DB_HOST=postgres DB_PORT=5432 DB_USER=postgres DB_PASSWORD=postgres DB_NAME=dodo_invoice
EXPOSE 8080
CMD ["/app/wait-and-migrate.sh"]
