#!/bin/bash

echo "Installing PQChat..."

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "Docker is not installed. Please install Docker first."
    exit 1
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    echo "Docker Compose is not installed. Please install Docker Compose first."
    exit 1
fi

# Create directory for PQChat
mkdir -p ~/pqchat
cd ~/pqchat

# Download docker-compose.yml
echo "Downloading configuration files..."
curl -O https://raw.githubusercontent.com/MNylif/PQChat/main/docker-compose.yml
curl -O https://raw.githubusercontent.com/MNylif/PQChat/main/pqchat-example.toml

# Rename config file
mv pqchat-example.toml pqchat.toml

# Create data directory
mkdir -p data

echo "Starting PQChat..."
docker-compose up -d

echo "PQChat is now running!"
echo "You can access it at http://localhost:6167"
echo "To view logs, run: docker-compose logs -f"
echo "To stop PQChat, run: docker-compose down"
