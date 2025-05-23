#!/bin/bash
# Script to install Docker and Docker Compose

echo "Installing Docker and Docker Compose..."
sudo apt update
sudo apt install -y docker.io docker-compose
sudo usermod -aG docker $USER

echo "Installation complete! Please log out and log back in for group changes to take effect."
echo "Then run: docker-compose up --build -d"
