#!/usr/bin/env bash
# Check retained packages through offline package managers. A previous version may
# be a 0.0.0 fixture made from the same archive, rather than a historic release.
set -euo pipefail
repo=$(realpath "${1:?repository directory required}")
version=${2:?version required}
expected=${3:?expected executable or tar.xz archive required}
engine=${CONTAINER_ENGINE:-podman}
repository_url=${PACKAGE_REPOSITORY_URL:-}
network=none
if [[ -n $repository_url ]]; then
  test "$repository_url" = https://packages.esdieff.com
  network=bridge
fi
[[ $version =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || exit 1
case $(uname -m) in
  x86_64) architecture=amd64 ;;
  aarch64|arm64) architecture=arm64 ;;
  *) echo 'Unsupported package test host' >&2; exit 1 ;;
esac
test -f "$repo/apt/pool/main/e/esdieff/esdieff_${version}_${architecture}.deb"
if [[ $expected == *.tar.xz ]]; then
  digest=$(tar -xOf "$expected" --wildcards '*/esdieff' | sha256sum | cut -d' ' -f1)
else
  digest=$(sha256sum "$expected" | cut -d' ' -f1)
fi
previous=$(find "$repo/apt/pool/main/e/esdieff" -name "esdieff_*_${architecture}.deb" -printf '%f\n' |
  sed -E "s/^esdieff_(.*)_${architecture}\.deb$/\1/" | sort -Vu |
  awk -v current="$version" '$0 == current {exit} {previous=$0} END {print previous}')

for image in docker.io/library/ubuntu:22.04 docker.io/library/ubuntu:24.04 docker.io/library/fedora:44; do
  "$engine" run --rm --network="$network" -i -v "$repo:/repository:ro" \
    -e VERSION="$version" -e PREVIOUS="$previous" -e DIGEST="$digest" \
    -e REPOSITORY_URL="$repository_url" \
    "$image" bash -s <<'CLIENT'
set -euo pipefail
if command -v apt-get >/dev/null; then
  if [[ -n $REPOSITORY_URL ]]; then
    apt-get -q update
    apt-get -qy --no-install-recommends install ca-certificates
  fi
  rm -f /etc/apt/sources.list /etc/apt/sources.list.d/*
  cp /repository/esdieff.gpg /usr/share/keyrings/esdieff.gpg
  printf 'deb [signed-by=/usr/share/keyrings/esdieff.gpg] %s/apt stable main\n' "${REPOSITORY_URL:-file:/repository}" > /etc/apt/sources.list.d/esdieff.list
  apt-get -q -o Acquire::Languages=none update
  if [[ -n $PREVIOUS ]]; then
    apt-get -qy install "esdieff=$PREVIOUS"
    test "$(dpkg-query -W -f='${Version}' esdieff)" = "$PREVIOUS"
  fi
  if [[ -n $PREVIOUS ]]; then
    apt-get -qy --only-upgrade install esdieff
  else
    apt-get -qy install "esdieff=$VERSION"
  fi
  test "$(dpkg-query -W -f='${Version}' esdieff)" = "$VERSION"
else
  generation=$(sed -n 's|^https://packages.esdieff.com/rpm/snapshots/\([^/]*\)/$|\1|p' /repository/rpm/mirrorlist)
  [[ $generation =~ ^[0-9a-f]{64}$ ]]
  cat > /etc/yum.repos.d/esdieff.repo <<REPO
[esdieff]
name=Esdieff
baseurl=${REPOSITORY_URL:-file:///repository}/rpm/snapshots/$generation/
enabled=1
gpgcheck=1
repo_gpgcheck=1
gpgkey=file:///repository/esdieff.asc
REPO
  flags=(-y --disablerepo='*' --enablerepo=esdieff)
  if [[ -n $PREVIOUS ]]; then
    dnf "${flags[@]}" install "esdieff-$PREVIOUS"
    test "$(rpm -q --qf '%{VERSION}' esdieff)" = "$PREVIOUS"
  fi
  if [[ -n $PREVIOUS ]]; then
    dnf "${flags[@]}" upgrade esdieff
  else
    dnf "${flags[@]}" install "esdieff-$VERSION"
  fi
  test "$(rpm -q --qf '%{VERSION}' esdieff)" = "$VERSION"
fi
test "$(command -v esdieff)" -ef /usr/bin/esdieff
test "$(sha256sum /usr/bin/esdieff | cut -d' ' -f1)" = "$DIGEST"
test "$(esdieff --version)" = "esdieff $VERSION"
printf '%s\n' '{"sdfObject":{"Thermostat":{"sdfProperty":{"temperature":{"type":"number"}}}}}' > /tmp/model.sdf.json
esdieff check /tmp/model.sdf.json
if command -v apt-get >/dev/null; then
  apt-get -qy remove esdieff
else
  dnf -y --disablerepo='*' remove esdieff
fi
test ! -e /usr/bin/esdieff
hash -r
if command -v esdieff >/dev/null; then echo 'Removal left the command on PATH' >&2; exit 1; fi
test -s /tmp/model.sdf.json
CLIENT
done
