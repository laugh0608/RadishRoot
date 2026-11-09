FROM debian@sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850

LABEL org.opencontainers.image.title="RadishRoot Debian image tools" \
      org.opencontainers.image.description="Pinned ARM64 GPT/FAT development tools; not a RadishRoot guest image" \
      org.opencontainers.image.base.name="debian@sha256:a2ca65377c251208de42e1a2722cb949211236c161b634f6f3663754e2d06850"

COPY Dockerfile build-image.sh packages.tsv base-packages.tsv snapshot.sources /usr/local/share/radishroot-debian-tools/build/
COPY snapshot.sources /etc/apt/sources.list.d/debian.sources
RUN /bin/sh /usr/local/share/radishroot-debian-tools/build/build-image.sh

WORKDIR /work
CMD ["/bin/sh"]
