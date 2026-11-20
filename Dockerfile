# Distroless multi-arch image for spriters.
#
# Built from pre-compiled MUSL binaries (not from source), so the build is
# fast and the image stays ~10 MB. Expects the build context to contain
# `spriters-linux-amd64` and `spriters-linux-arm64` next to this
# file — populate them with `just docker-context VERSION` or fetch them
# from a Codeberg release.
#
# Build locally for the host platform:
#     just docker-context v0.2.0
#     docker build -t spriters:dev .
#
# Build multi-arch via buildx (CI uses this):
#     docker buildx build --platform linux/amd64,linux/arm64 -t spriters:v0.2.0 .

FROM gcr.io/distroless/static-debian12:nonroot

ARG TARGETARCH
COPY spriters-linux-${TARGETARCH} /usr/local/bin/spriters

USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/spriters"]
