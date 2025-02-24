#!/bin/bash

echo "Installing PQChat..."

# Function to install Docker on Ubuntu
install_docker() {
    echo "Installing Docker..."
    # Remove old versions if they exist
    apt-get remove -y docker docker-engine docker.io containerd runc || true
    
    # Update package index
    apt-get update
    
    # Install required packages
    apt-get install -y \
        apt-transport-https \
        ca-certificates \
        curl \
        gnupg \
        lsb-release

    # Add Docker's official GPG key
    mkdir -p /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg

    # Set up the repository
    echo \
        "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
        $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null

    # Update package index again
    apt-get update

    # Install Docker Engine
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

    # Start and enable Docker
    systemctl start docker
    systemctl enable docker

    echo "Docker installed successfully!"
}

# Function to install Docker Compose
install_docker_compose() {
    echo "Installing Docker Compose..."
    curl -L "https://github.com/docker/compose/releases/download/v2.23.3/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose
    chmod +x /usr/local/bin/docker-compose
    echo "Docker Compose installed successfully!"
}

# Check if script is run as root
if [ "$EUID" -ne 0 ]; then 
    echo "Please run as root (use sudo)"
    exit 1
fi

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    install_docker
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    install_docker_compose
fi

# Create directory for PQChat
INSTALL_DIR="/opt/pqchat"
echo "Creating installation directory at $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cd "$INSTALL_DIR"

# Download configuration files
echo "Downloading configuration files..."
curl -O https://raw.githubusercontent.com/MNylif/PQChat/main/docker-compose.yml
curl -O https://raw.githubusercontent.com/MNylif/PQChat/main/pqchat-example.toml
curl -O https://raw.githubusercontent.com/MNylif/PQChat/main/Dockerfile

# Rename config file
cp pqchat-example.toml pqchat.toml

# Create data directory with proper permissions
mkdir -p data
chown -R 1000:1000 data

echo "Starting PQChat..."
docker-compose up -d

# Check if containers are running
if docker-compose ps | grep -q "running"; then
    echo "PQChat is now running!"
    echo "You can access it at http://localhost:6167"
    echo "Configuration files are in $INSTALL_DIR"
    echo ""
    echo "Management commands (run from $INSTALL_DIR):"
    echo "- View logs: docker-compose logs -f"
    echo "- Stop server: docker-compose down"
    echo "- Start server: docker-compose up -d"
    echo "- Restart server: docker-compose restart"
else
    echo "Error: PQChat failed to start. Checking logs..."
    docker-compose logs
    exit 1
fi
