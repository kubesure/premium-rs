FROM rust:latest as builder
WORKDIR /app
COPY . .
RUN cargo build --release
FROM debian:bullseye
RUN apt-get update 
RUN apt-get install net-tools
RUN apt-get install -y iputils-ping
ARG LISTEN_PORT=8080
ENV LISTEN_PORT ${LISTEN_PORT}
COPY --from=builder /app/target/release/premium-rs ./premium-rs
COPY --from=builder /app/premium_tables.xlsx ./premium_tables.xlsx
CMD ["./premium-rs"]
EXPOSE 8000