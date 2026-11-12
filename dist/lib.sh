# Shared functions of the dist scripts. This file is sourced, not executed.
#
# A product (app, api, logs) is built in two phases: stage_<product> creates the directory that is
# the content of the distribution (for the macOS app, the .app bundle in a disk image folder),
# pack_<product> creates the archive from it. The functions for the products are in
# product-<product>.sh.

set -e

basedir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
distdir="${basedir}/dist"
profile=release-dist

# Sets version, target, label and universal. The version is the one in Cargo.toml, the only place
# where it is defined (see dist/bump-version); the optional argument is the target. Without a target, things are built for the host, except on macOS, where binaries are universal.
# The label names the platform in archive names.
init_target() {
  version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "${basedir}/Cargo.toml" | head -n 1)"
  target=${1}
  if [ -z "${version}" ]; then
    >&2 echo "error: could not read the version from Cargo.toml"
    return 1
  fi
  universal=false
  if [ -z "${target}" ]; then
    target="$(rustc -vV | sed -n "s/^host: \(.*\)/\1/p")"
    case "${target}" in
      *-apple-darwin) universal=true ;;
    esac
  fi
  if ${universal}; then
    label=universal-apple-darwin
  else
    label=${target}
  fi
}

# Binaries for macOS run on the versions that the app declares as minimum.
export MACOSX_DEPLOYMENT_TARGET=15.0

# Builds the given cargo packages for the current target (both architectures if universal).
build_packages() {
  local packages=()
  local package
  for package in "$@"; do
    packages+=(--package "${package}")
  done
  pushd "${basedir}" >/dev/null
  if ${universal}; then
    cargo build --target aarch64-apple-darwin --profile "${profile}" "${packages[@]}"
    cargo build --target x86_64-apple-darwin --profile "${profile}" "${packages[@]}"
  else
    cargo build --target "${target}" --profile "${profile}" "${packages[@]}"
  fi
  popd >/dev/null
}

# Copies a built file (e.g. a binary or library) to <destination>, joining the architectures if
# universal.
install_built() { # <file name in the target directory> <destination>
  if ${universal}; then
    lipo -create -output "${2}" \
      "${basedir}/target/aarch64-apple-darwin/${profile}/${1}" \
      "${basedir}/target/x86_64-apple-darwin/${profile}/${1}"
  else
    cp "${basedir}/target/${target}/${profile}/${1}" "${2}"
  fi
}

# Creates <directory>.tar.bz2 from a directory.
make_tarball() {
  tar -cjf "${1}.tar.bz2" -C "$(dirname "${1}")" "$(basename "${1}")"
}
