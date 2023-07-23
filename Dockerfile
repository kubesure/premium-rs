FROM rust:1.68 AS builder
COPY . .
RUN cargo build --release

FROM debian:bullseye
COPY --from=builder ./premium_tables.xlsx ./target/release/premium_tables.xlsx
COPY --from=builder ./target/release/premium-rs ./target/release/premium-rs
CMD ["/target/release/premium-rs"]