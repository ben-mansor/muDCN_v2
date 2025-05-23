FROM ubuntu:22.04

# Configure apt to ignore signature verification issues
RUN echo 'APT::Get::AllowUnauthenticated "true";' > /etc/apt/apt.conf.d/99allow-unauthenticated
RUN echo 'Acquire::Check-Valid-Until "false";' > /etc/apt/apt.conf.d/99no-check-valid-until
RUN echo 'Acquire::AllowInsecureRepositories "true";' > /etc/apt/apt.conf.d/99allow-insecure-repositories
RUN echo 'Acquire::AllowDowngradeToInsecureRepositories "true";' > /etc/apt/apt.conf.d/99allow-downgrade-to-insecure

# Configure for non-interactive installation
ENV DEBIAN_FRONTEND=noninteractive
ENV TZ=Europe/Berlin

# Install minimal packages
RUN apt-get update --allow-unauthenticated && \
    apt-get install -y --allow-unauthenticated \
    curl \
    python3 \
    && rm -rf /var/lib/apt/lists/*

# Simple command to verify build
CMD ["echo", "Test build successful"]
