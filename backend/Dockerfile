FROM rust:1.78 AS builder

WORKDIR /src
COPY . .

RUN cargo build --release

FROM rust:1.78 as shatterbird-cli
COPY --from=builder /src/target/release/shatterbird-server /shatterbird-cli
CMD [ "/shatterbird-cli" ]

FROM rust:1.78 as shatterbird-indexer
COPY --from=builder /src/target/release/shatterbird-indexer /shatterbird-indexer
CMD [ "/shatterbird-indexer" ]

FROM rust:1.78 as shatterbird-server
COPY --from=builder /src/target/release/shatterbird-server /shatterbird-server
CMD [ "/shatterbird-server" ]
