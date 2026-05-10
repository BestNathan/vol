# Build with: docker build --build-arg BIN_NAME=docs-rs-mcp -f crates/vol-mcp-servers/Dockerfile .
#
# The binary is built on the host via `cargo build --release`, then this
# Dockerfile packages it into a minimal runtime image.
#
# Uses Ubuntu (glibc) because the host-compiled binary is dynamically linked
# against glibc. Alpine (musl) is incompatible without cross-compilation.
#
# The ENV BIN_NAME + ENTRYPOINT shell pattern allows the same Dockerfile to
# work with any binary name passed via --build-arg, since Docker ARG does not
# persist at runtime.

FROM ubuntu:24.04
ARG BIN_NAME=docs-rs-mcp

# Make the binary name available at runtime (ARG does not persist)
ENV BIN_NAME=${BIN_NAME}

COPY target/release/${BIN_NAME} /usr/local/bin/${BIN_NAME}

EXPOSE 8080
ENTRYPOINT ["/bin/sh", "-c", "/usr/local/bin/${BIN_NAME} \"$@\"", "--"]
CMD ["--http", "0.0.0.0:8080"]
