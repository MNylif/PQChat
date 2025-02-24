# PQChat

PQChat is a secure, post-quantum encrypted Matrix homeserver implementation in Rust. It focuses on providing robust end-to-end encryption using post-quantum cryptographic algorithms to ensure future-proof security against quantum computer attacks.

## Features

- Post-quantum cryptography implementation for end-to-end encryption
- Written in pure Rust for memory safety and performance
- Multi-platform support (x86_64 and ARM64)
- Docker support for easy deployment
- Matrix protocol compatibility
- Modern and efficient database backend

## Security Features

- Post-quantum secure key exchange
- Forward secrecy
- Perfect forward secrecy
- End-to-end encryption
- Quantum-resistant algorithms

## Quick Start with Docker

The fastest way to get started with PQChat is using Docker. Run this command:

```bash
curl -fsSL https://raw.githubusercontent.com/MNylif/PQChat/main/install.sh | bash
```

This will:
- Check for Docker and Docker Compose
- Create necessary directories
- Download and set up configuration files
- Start PQChat using Docker Compose

After installation, PQChat will be available at `http://localhost:6167`.

For users who prefer to inspect the installation script first:
```bash
# Download the script
curl -O https://raw.githubusercontent.com/MNylif/PQChat/main/install.sh
# Review it
cat install.sh
# Make it executable and run
chmod +x install.sh
./install.sh
```

### Docker Management Commands

- View logs: `docker-compose logs -f`
- Stop server: `docker-compose down`
- Start server: `docker-compose up -d`
- Restart server: `docker-compose restart`

## Building from Source

### Prerequisites

- Rust toolchain (1.70.0 or newer)
- Docker (optional, for containerized deployment)
- Build essentials (pkg-config, gcc, etc.)

### Build Steps

1. Clone the repository:
   ```bash
   git clone https://github.com/MNylif/PQChat.git
   cd PQChat
   ```

2. Build using Cargo:
   ```bash
   cargo build --release
   ```

### Docker Build

1. Build the Docker image:
   ```bash
   docker-compose build
   ```

2. Run the container:
   ```bash
   docker-compose up
   ```

## Configuration

1. Copy the example configuration:
   ```bash
   cp pqchat-example.toml pqchat.toml
   ```

2. Edit the configuration file to match your setup:
   ```bash
   nano pqchat.toml
   ```

## License

This project is licensed under the AGPL-3.0 License - see the [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
