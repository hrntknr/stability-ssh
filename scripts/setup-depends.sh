#!/bin/bash -e
cd $(dirname $0)
function _sudo() {
  if [ -z "$NOSUDO" ]; then
    sudo $@
  else
    $@
  fi
}

case $(uname -s) in
Linux)
  _sudo apt-get update
  _sudo apt-get install -y unzip musl-dev musl-tools curl jq $(./resolve-arch.sh gcc)
  _sudo ./setup-protobuf.sh
  ;;
Darwin)
  brew list jq &>/dev/null || brew install jq
  brew list curl &>/dev/null || brew install curlg
  _sudo ./setup-protobuf.sh
  ;;
*)
  echo "unsupported platform"
  exit 1
  ;;
esac
