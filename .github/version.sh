#!/bin/sh
set -eu

APP="src Cargo.toml Cargo.lock rust-toolchain.toml .cargo"

fail() {
    echo "::error::$1"
    exit 1
}

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
latest=$(git tag --list 'v*' --sort=-v:refname | head -n 1 | sed 's/^v//')

newer() {
    [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1)" = "$1" ]
}

check() {
    changed=$(git diff --name-only "$1"...HEAD -- $APP)
    if [ -z "$changed" ]; then
        echo "no app changes, no new version needed"
        return
    fi
    if [ -n "$latest" ] && ! newer "$version" "$latest"; then
        fail "this pull request changes the app, so it needs a new version: set version in Cargo.toml above $latest (the last release) and add its section to CHANGELOG.md"
    fi
    grep -q "^## $version\$" CHANGELOG.md || fail "add a '## $version' section to CHANGELOG.md with what changed for users"
    echo "this pull request releases $version"
}

release() {
    tag="v$version"
    if git rev-parse -q --verify "refs/tags/$tag" >/dev/null; then
        changed=$(git diff --name-only "$tag" HEAD -- $APP)
        [ -z "$changed" ] || fail "$tag is already released, but the app changed since; bump the version in a new pull request"
        echo "$tag is already released"
        return
    fi
    git tag "$tag"
    git push origin "$tag"
    gh workflow run release.yml --ref "$tag" -f tag="$tag"
    echo "releasing $tag"
}

case "${1:-}" in
    check) check "$2" ;;
    release) release ;;
    *) fail "usage: version.sh check <base> | version.sh release" ;;
esac
