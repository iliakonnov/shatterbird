FROM rust:1.78 AS builder

WORKDIR /src
COPY . .

RUN cargo build --release

FROM rust:1.78 as shatterbird-cli
RUN apt-get update && apt-get install -y graphviz
COPY --from=builder /src/target/release/shatterbird-server /shatterbird-cli
ENTRYPOINT ["/shatterbird-cli"]

FROM rust:1.78 as shatterbird-indexer
COPY --from=builder /src/target/release/shatterbird-indexer /shatterbird-indexer
ENTRYPOINT ["/shatterbird-indexer"]

FROM rust:1.78 as shatterbird-server
COPY --from=builder /src/target/release/shatterbird-server /shatterbird-server
ENTRYPOINT ["/shatterbird-server"]
