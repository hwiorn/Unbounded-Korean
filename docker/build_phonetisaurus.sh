#!/usr/bin/env bash
# Builds the original C++ Phonetisaurus toolchain from source, pinned to OpenFst 1.7.2
# (the last version confirmed to work — Phonetisaurus's build breaks against OpenFst
# 1.8+, an unresolved upstream issue since 2021: AdolfVonKleist/Phonetisaurus#70).
#
# This is the primary path. The caller (docker/phonetisaurus-train.Dockerfile) falls
# back to `pip install phonetisaurus` (rhasspy's prebuilt manylinux1_x86_64 wheel) if
# this script fails, since this image targets linux/amd64.
set -euo pipefail

JOBS="$(nproc)"

cd /build
curl -fsSL http://www.openfst.org/twiki/pub/FST/FstDownload/openfst-1.7.2.tar.gz -o openfst.tar.gz
tar xzf openfst.tar.gz
cd openfst-1.7.2
./configure --enable-static --enable-shared --enable-far --enable-ngram-fsts
make -j"$JOBS"
make install
ldconfig

cd /build
git clone --depth 1 https://github.com/mitlm/mitlm.git
cd mitlm
./autogen.sh
./configure
make -j"$JOBS"
make install
ldconfig

cd /build
git clone --depth 1 https://github.com/AdolfVonKleist/Phonetisaurus.git
cd Phonetisaurus
./configure --with-openfst-includes=/usr/local/include --with-openfst-libs=/usr/local/lib
make -j"$JOBS"
make install
ldconfig

phonetisaurus-train --help >/dev/null
