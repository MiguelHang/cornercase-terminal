#!/bin/sh
set -eu

url="https://github.com/usecornercase/cornercase-terminal/releases/latest/download/cornercase-installer.sh"
installer=$(mktemp)
trap 'rm -f "$installer"' EXIT

if command -v curl >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -fsSL "$url" -o "$installer"
elif command -v wget >/dev/null 2>&1; then
    wget --https-only -q "$url" -O "$installer"
else
    echo "cornercase: install curl or wget first" >&2
    exit 1
fi

sh "$installer" "$@"
