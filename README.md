# Stable SSH

StableSSH is a software to stabilize SSH communication by wrapping SSH with quic.

## Features

- Resistant to long communication breaks. (e.g., client terminal sleep)
  - Encap SSH with quic to increase stability.
  - There is an internal buffer to retry and retransmit connections.

## Similar Softwares

It is influenced by the following software, but differs in some respects.

- quicssh-rs
  - The point to encap ssh with quic is the same.
  - Because quicssh-rs has no internal buffer or retry function, there is no regime for long communication breaks (e.g., client sleep).
- mosh
  - The same internal buffer and retry function are retained, making it tolerant of long communication breaks.
  - It is not ssh, so port forwarding, file transfer, vscode remote, etc. are not available.

## Usage

### Client config (sshconfig)

```
Host target
  Port 2222
  ProxyCommand stablessh client %h:%p
```

### Server daemon (systemd)

```
[Unit]
Description=stablessh Daemon

[Service]
Type=simple
ExecStart=/usr/local/bin/stablessh server
[Install]
WantedBy=multi-user.target
```

### Options

```
> $ stablessh --help
Usage: stablessh <COMMAND>

Commands:
  server
  client
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

> $ stablessh client --help
Usage: stablessh client [OPTIONS] <TARGET>

Arguments:
  <TARGET>

Options:
  -i, --idle <IDLE>            [default: 3]
  -k, --keepalive <KEEPALIVE>  [default: 1]
  -b, --bufsize <BUFSIZE>      [default: 32]
  -4, --only-ipv4
  -6, --only-ipv6
  -h, --help                   Print help

> $ stablessh server --help
Usage: stablessh server [OPTIONS]

Options:
  -i, --idle <IDLE>            [default: 3]
  -k, --keepalive <KEEPALIVE>  [default: 1]
  -b, --bufsize <BUFSIZE>      [default: 32]
  -l, --listen <LISTEN>        [default: 0.0.0.0:2222]
  -f, --forward <FORWARD>      [default: localhost:22]
  -h, --help                   Print help
```

## About bufsize

bufsize specifies the bit size of the buffer. (upper limit 32)
The default value allows a packet to be buffered for 32-bit space, but it may consume infinite memory.
If memory usage is a concern, try reducing bufsize.
