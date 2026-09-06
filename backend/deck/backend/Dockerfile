# Cave Deck backend — multi-stage build (spec §3.2: single static binary)
FROM rust:1.98-slim AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM gcr.io/distroless/cc-debian12
COPY --from=build /build/target/release/cave-deck /usr/local/bin/cave-deck
ENV CAVE_DECK_PORT=7780
EXPOSE 7780
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/cave-deck"]
