#!/bin/sh
# Set up the signed Esdieff package repository and install the command.
set -eu

as_root() {
    if [ "$(id -u)" = 0 ]; then
        "$@"
    else
        sudo "$@"
    fi
}

main() {
    case "$(uname -s):$(uname -m)" in
        Linux:x86_64|Linux:aarch64|Linux:arm64) ;;
        *) echo 'Use the installation guide for your platform: https://esdieff.com/docs/getting-started/' >&2; exit 1 ;;
    esac

    if command -v apt-get >/dev/null 2>&1; then
        manager=apt
    elif command -v dnf >/dev/null 2>&1; then
        manager=dnf
    else
        echo 'This installer supports Debian, Ubuntu and Fedora. Downloads: https://github.com/pederbe/esdieff/releases/latest' >&2
        exit 1
    fi
    if [ "$(id -u)" != 0 ] && ! command -v sudo >/dev/null 2>&1; then
        echo 'Install sudo or run this installer as root.' >&2
        exit 1
    fi

    work=$(mktemp -d)
    trap 'rm -rf "$work"' EXIT
    trap 'exit 1' HUP INT TERM
    curl -fsSL https://packages.esdieff.com/esdieff.asc -o "$work/esdieff.asc"
    # Pin the published key so no manual fingerprint check is needed.
    if ! printf '%s  %s\n' \
        85b0852c9e158e495e65e857c2037f8d1b5275d8f94ad1bc39a12c2f61aa387e \
        "$work/esdieff.asc" | sha256sum --check --status; then
        echo 'The Esdieff signing key could not be verified. Installation stopped.' >&2
        exit 1
    fi

    if [ "$manager" = apt ]; then
        printf '%s\n' 'deb [signed-by=/usr/share/keyrings/esdieff.asc] https://packages.esdieff.com/apt stable main' > "$work/esdieff.list"
        as_root install -D -m 644 "$work/esdieff.asc" /usr/share/keyrings/esdieff.asc
        as_root install -D -m 644 "$work/esdieff.list" /etc/apt/sources.list.d/esdieff.list
        as_root apt-get update
        as_root apt-get install -y esdieff
    else
        cat > "$work/esdieff.repo" <<'REPO'
[esdieff]
name=Esdieff
mirrorlist=https://packages.esdieff.com/rpm/mirrorlist
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=file:///etc/pki/rpm-gpg/RPM-GPG-KEY-esdieff
REPO
        as_root install -D -m 644 "$work/esdieff.asc" /etc/pki/rpm-gpg/RPM-GPG-KEY-esdieff
        as_root rpm --import /etc/pki/rpm-gpg/RPM-GPG-KEY-esdieff
        as_root install -D -m 644 "$work/esdieff.repo" /etc/yum.repos.d/esdieff.repo
        as_root dnf install -y --refresh esdieff
    fi
    printf '\nEsdieff is installed. Try: esdieff --help\n'
}

# Read the complete installer before making changes when piped into sh.
main
