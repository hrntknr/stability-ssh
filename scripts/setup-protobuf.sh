#!/bin/bash -e

PROTOC_VERSION="26.0"

if ! command -v curl >/dev/null; then
  echo "curl is required"
  exit 1
fi
if ! command -v jq >/dev/null; then
  echo "jq is required"
  exit 1
fi

case $(uname -s),$(uname -m) in
Linux,x86_64)
  platform=linux-x86_64
  ;;
Linux,aarch64)
  platform=linux-aarch_64
  ;;
Darwin,x86_64)
  platform=osx-x86_64
  ;;
Darwin,arm64)
  platform=osx-aarch_64
  ;;
*)
  echo "unsupported platform"
  exit 1
  ;;
esac

tmp=$(mktemp -d)
curl -sfL https://github.com/protocolbuffers/protobuf/releases/download/v$PROTOC_VERSION/protoc-$PROTOC_VERSION-$platform.zip -o $tmp/protoc.zip
unzip -d $tmp $tmp/protoc.zip
mv $tmp/bin/protoc /usr/local/bin
