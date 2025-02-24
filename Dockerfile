FROM rust:slim as builder

WORKDIR /usr/src/pqchat

# Install build dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    git \
    && rm -rf /var/lib/apt/lists/*

# Set environment variables for the build
ENV RUSTFLAGS="-C target-feature=-neon,-sha3"
ENV PQC_DISABLE_ASM=1

# First copy only files needed for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create workspace structure
RUN mkdir -p src/crypto/src && \
    echo 'pub fn main() { println!("placeholder"); }' > src/crypto/src/lib.rs && \
    mkdir -p src && \
    echo 'fn main() { println!("placeholder"); }' > src/main.rs

# Copy workspace member Cargo.toml
COPY src/crypto/Cargo.toml src/crypto/

# Build dependencies
RUN cargo build --release

# Remove the placeholder files
RUN rm -rf src

# Now copy the real source code
COPY . .

# Build the application
RUN cargo build --release

FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN useradd -m -U -s /bin/false pqchat

# Create necessary directories
RUN mkdir -p /var/lib/pqchat && \
    chown -R pqchat:pqchat /var/lib/pqchat

# Copy the binary
COPY --from=builder /usr/src/pqchat/target/release/pqchat /usr/local/bin/

# Set up volumes and expose port
VOLUME ["/var/lib/pqchat"]
EXPOSE 6167

# Switch to non-root user
USER pqchat

# Run the server
CMD ["pqchat"]
