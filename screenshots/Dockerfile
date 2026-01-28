# Playwright container for screenshot automation
FROM mcr.microsoft.com/playwright:v1.58.0-noble

WORKDIR /work

# Copy entrypoint script
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Default command - keep container running for exec
ENTRYPOINT ["/entrypoint.sh"]
CMD ["tail", "-f", "/dev/null"]
