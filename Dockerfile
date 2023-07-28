FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release
FROM debian:bullseye
RUN apt-get update 
RUN apt-get install net-tools
COPY --from=builder /app/target/release/premium-rs ./premium-rs
COPY --from=builder /app/premium_tables.xlsx ./premium_tables.xlsx
CMD ["./premium-rs"]
EXPOSE 8000