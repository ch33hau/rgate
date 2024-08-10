# Use an official Rust image as a base
FROM rust:latest AS builder

# Set the working directory inside the container
WORKDIR /usr/src/rgate

# Copy the source code into the container
COPY . .

# Build the application in release mode
RUN cargo build --release

# Use a minimal base image with the Rust binary
FROM debian:buster-slim

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/rgate/target/release/rgate /usr/local/bin/rgate

# Expose the ports that your application will run on
EXPOSE 9000 9001

# Set the entry point to the binary
ENTRYPOINT ["rgate"]

# Pass the necessary arguments
CMD ["--url", "https://example.com", "--port", "9000", "--dashboard-port", "9001"]
