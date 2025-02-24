FROM debian:bookworm-slim

# Create non-root user
RUN groupadd -r pqchat && useradd -r -g pqchat pqchat

# Install dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create directory structure
RUN mkdir -p /var/lib/pqchat && \
    chown -R pqchat:pqchat /var/lib/pqchat

# Copy the pre-built binary
COPY --chown=pqchat:pqchat target/release/pqchat /usr/local/bin/

# Set working directory
WORKDIR /var/lib/pqchat

# Switch to non-root user
USER pqchat

# Expose port
EXPOSE 6167

# Set entrypoint
ENTRYPOINT ["/usr/local/bin/pqchat"]
