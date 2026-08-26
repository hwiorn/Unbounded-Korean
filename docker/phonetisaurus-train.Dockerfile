# Training-only image for the korean-transliteration Phonetisaurus pipeline. Not part
# of the runtime crate — crates/korean-transliteration has zero Docker/C++ dependency,
# it only loads the .fst files this image produces (see
# docs/specs/2026-08-26-korean-transliteration-design.md).
#
# Must be built for linux/amd64: the fallback path below installs rhasspy's
# manylinux1_x86_64 `phonetisaurus` wheel, which only exists for that platform.
FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential autoconf automake libtool pkg-config \
    git curl ca-certificates python3 python3-pip \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
COPY docker/build_phonetisaurus.sh .
# Primary path: build the original C++ toolchain from source (pinned OpenFst 1.7.2).
# Fallback path: rhasspy's prebuilt PyPI wheel, if the from-source build fails for any
# reason (e.g. an upstream URL going stale) — this image is linux/amd64, so the wheel
# applies natively, no emulation needed.
RUN bash build_phonetisaurus.sh || pip3 install --break-system-packages phonetisaurus

WORKDIR /work
