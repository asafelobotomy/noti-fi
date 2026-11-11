#!/bin/sh
# Packaging check for the Notifications page: the package, the release archive and its install.sh put exactly the files of
# uq/pages/noti-fi/ under /usr/share/uq/settings/pages/noti-fi/ (and nothing else under /usr/share/uq), and a ~/.local
# install puts none anywhere, because uQ Settings reads pages from /usr/share/uq only.
#   sh tests/uq/install_test.sh
# Runs packaging/PKGBUILD's package(), packaging/build-tarball.sh and packaging/install.sh on stand-in binaries in a
# temporary folder. Needs bash, tar and coreutils.
set -eu
cd "$(dirname "$0")/../.."
root=$(pwd)
ver=0.0.0-uqtest
arch=$(uname -m)
had_dist=no
[ -d "$root/dist" ] && had_dist=yes
tmp=$(mktemp -d)
cleanup() {
    rm -rf "$tmp" "$root/dist/noti-fi-$ver-$arch-linux" "$root/dist/noti-fi-$ver-$arch-linux.tar.gz" "$root/dist/noti-fi-$ver-$arch-linux.tar.gz.sha256"
    [ "$had_dist" = yes ] || rmdir "$root/dist" 2>/dev/null || true
}
trap cleanup EXIT
unset XDG_STATE_HOME XDG_DATA_HOME XDG_CONFIG_HOME DESTDIR PREFIX

fail() {
    echo "FAIL: $*" >&2
    exit 1
}

want=$(cd uq/pages/noti-fi && ls -A | sort)
[ -n "$want" ] || fail "uq/pages/noti-fi is empty"
for f in page.toml Page.qml logic.js; do
    echo "$want" | grep -qx "$f" || fail "uq/pages/noti-fi has no $f"
done

# $1 holds exactly the page folder's files, unchanged
same_as_page() {
    [ -d "$1" ] || fail "$1 is missing"
    got=$(cd "$1" && ls -A | sort)
    [ "$got" = "$want" ] || fail "$1 holds: $(echo "$got" | tr '\n' ' ') but the page folder holds: $(echo "$want" | tr '\n' ' ')"
    for f in $want; do
        cmp -s "uq/pages/noti-fi/$f" "$1/$f" || fail "$1/$f differs from uq/pages/noti-fi/$f"
    done
}
# nothing but the page under $1/share/uq
only_the_page() {
    n=$(cd "$1/share/uq" && find . -type f | wc -l)
    [ "$n" -eq "$(echo "$want" | wc -l)" ] || fail "$1/share/uq holds $n files, not just the page"
}

# ---- 1. the package: packaging/PKGBUILD package()
pkgver=$(sed -n 's/^pkgver=//p' packaging/PKGBUILD)
tree="$tmp/src/noti-fi-$pkgver"
mkdir -p "$tree/target/release"
cp -r packaging uq LICENSE README.md "$tree/"
for b in notify notify-panel; do
    printf '#!/bin/sh\nexit 0\n' > "$tree/target/release/$b"
    chmod +x "$tree/target/release/$b"
done
bash -c 'set -eu; . "$1"; srcdir=$2; pkgdir=$3; cd "$srcdir"; package; printf "%s\n" "${optdepends[@]}"' _ packaging/PKGBUILD "$tmp/src" "$tmp/pkg" > "$tmp/optdepends"
grep -qx 'uq: the Notifications page in uQ Settings' "$tmp/optdepends" || fail "PKGBUILD optdepends has no uq line"
same_as_page "$tmp/pkg/usr/share/uq/settings/pages/noti-fi"
only_the_page "$tmp/pkg/usr"

# ---- 2. the release archive
mkdir -p "$tmp/bin" "$tmp/target/release" "$tmp/x"
for b in notify notify-panel; do
    printf '#!/bin/sh\nexit 0\n' > "$tmp/target/release/$b"
    chmod +x "$tmp/target/release/$b"
done
printf '#!/bin/sh\necho 6.8.0\n' > "$tmp/bin/pkg-config"
chmod +x "$tmp/bin/pkg-config"
PATH="$tmp/bin:$PATH" packaging/build-tarball.sh "$ver" "$tmp/target" > /dev/null 2> "$tmp/tarball.err" || {
    cat "$tmp/tarball.err" >&2
    fail "build-tarball.sh failed"
}
tar -xzf "dist/noti-fi-$ver-$arch-linux.tar.gz" -C "$tmp/x"
unpacked="$tmp/x/noti-fi-$ver-$arch-linux"
same_as_page "$unpacked/share/uq/settings/pages/noti-fi"
only_the_page "$unpacked"

# ---- 3. install.sh, system-wide: the page goes in; --uninstall takes it out again
PATH="$tmp/bin:$PATH" HOME="$tmp/home" PREFIX=/usr DESTDIR="$tmp/root" "$unpacked/install.sh" > /dev/null
same_as_page "$tmp/root/usr/share/uq/settings/pages/noti-fi"
only_the_page "$tmp/root/usr"
[ -x "$tmp/root/usr/bin/notify" ] || fail "install.sh did not install notify"
# an install over an older one drops what the page no longer has
touch "$tmp/root/usr/share/uq/settings/pages/noti-fi/Old.qml"
PATH="$tmp/bin:$PATH" HOME="$tmp/home" PREFIX=/usr DESTDIR="$tmp/root" "$unpacked/install.sh" > /dev/null
same_as_page "$tmp/root/usr/share/uq/settings/pages/noti-fi"
HOME="$tmp/home" PREFIX=/usr DESTDIR="$tmp/root" "$unpacked/install.sh" --uninstall > /dev/null
[ ! -e "$tmp/root/usr/share/uq/settings/pages/noti-fi" ] || fail "--uninstall left the page"
[ ! -e "$tmp/root/usr/bin/notify" ] || fail "--uninstall left notify"

# ---- 4. install.sh into the home folder (and any other prefix): no page, because uQ would never read it
PATH="$tmp/bin:$PATH" HOME="$tmp/home2" "$unpacked/install.sh" > /dev/null
[ -x "$tmp/home2/.local/bin/notify" ] || fail "the home install has no notify"
[ ! -e "$tmp/home2/.local/share/uq" ] || fail "the home install put something under share/uq"
PATH="$tmp/bin:$PATH" HOME="$tmp/home3" PREFIX="$tmp/other" "$unpacked/install.sh" > /dev/null
[ ! -e "$tmp/other/share/uq" ] || fail "an install to another prefix put something under share/uq"

echo "ok: the page is packaged, archived and installed system-wide only"
