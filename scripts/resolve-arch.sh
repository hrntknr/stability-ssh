#!/bin/bash -eu

function normalize_arch() {
  case $1 in
  x86_64)
    echo x86_64
    ;;
  amd64)
    echo x86_64
    ;;
  aarch64)
    echo aarch64
    ;;
  arm64)
    echo aarch64
    ;;
  *)
    echo "unsupported arch"
    exit 1
    ;;
  esac
}

KERNEL=$(uname -s)
LIBC=${LIBC:-gnu}
ARCH=$(normalize_arch ${TARGETARCH:-$(uname -m)})

case $1,$KERNEL in
gcc,Linux)
  echo gcc-${ARCH//_/-}-linux-gnu
  ;;
target,Linux)
  echo ${ARCH}-unknown-linux-${LIBC}
  ;;
target,Darwin)
  echo ${ARCH}-apple-darwin
  ;;
*)
  echo "unsupported platform"
  exit 1
  ;;
esac
