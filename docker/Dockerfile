# PhotoSee standalone server (LAN, read-only roots). The build context is
# staged by scripts/docker-stage.sh, not the repo root:
#   photosee-server   static x86_64-unknown-linux-musl binary
#   web/              built UI (web/dist)
# FROM scratch on purpose: the binary is static, so the image needs no base
# layer and the Docker host needs no registry access to build it ($LAN_HOST,
# the first deployment, pulls from Docker Hub at tens of KB/s).
FROM scratch
# Product version (docker-stage.sh writes VERSION; pass it as a build arg).
ARG VERSION=unknown
LABEL org.opencontainers.image.title="PhotoSee" org.opencontainers.image.version="$VERSION"
COPY photosee-server /photosee-server
COPY web /web
ENV PHOTOSEE_WEB=/web \
    PHOTOSEE_DATA=/data \
    PHOTOSEE_PORT=8080
EXPOSE 8080
# Unprivileged: the roots are mounted read-only and /data must be owned by
# 65534 on the host.
USER 65534:65534
ENTRYPOINT ["/photosee-server"]
