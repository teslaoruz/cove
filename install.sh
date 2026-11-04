#!/bin/sh
# Install or update Cove on Linux, for the current user (no root needed).
#   curl -fsSL https://github.com/teslaoruz/cove/releases/latest/download/install.sh | sh
# Set COVE_VERSION=v0.2.0 to install a given release instead of the newest.
set -eu

main() {
    repo=teslaoruz/cove
    asset=cove-linux-x86_64.tar.gz

    case "$(uname -s)-$(uname -m)" in
        Linux-x86_64 | Linux-amd64) ;;
        *)
            echo "cove: there is no prebuilt Cove for $(uname -s) $(uname -m)." >&2
            echo "cove: build it from source: https://github.com/$repo#build-from-source" >&2
            exit 1
            ;;
    esac
    for tool in curl tar sha256sum; do
        command -v "$tool" >/dev/null 2>&1 || { echo "cove: '$tool' is needed but not installed." >&2; exit 1; }
    done

    if [ -n "${COVE_VERSION:-}" ]; then
        url="https://github.com/$repo/releases/download/$COVE_VERSION"
    else
        url="https://github.com/$repo/releases/latest/download"
    fi

    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT INT TERM

    echo "Downloading Cove..."
    curl -fsSL -o "$tmp/$asset" "$url/$asset"
    curl -fsSL -o "$tmp/SHA256SUMS" "$url/SHA256SUMS"
    if ! (cd "$tmp" && grep " $asset\$" SHA256SUMS | sha256sum -c - >/dev/null 2>&1); then
        echo "cove: the download does not match its checksum. Nothing was installed." >&2
        exit 1
    fi

    tar -xzf "$tmp/$asset" -C "$tmp"
    sh "$tmp/cove/install.sh"
}

# everything runs from here, so a half-downloaded script does nothing
main "$@"
