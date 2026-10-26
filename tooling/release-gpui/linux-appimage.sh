#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/common.sh"
REPO_ROOT="$(release_gpui_repo_root)"
VERSION="${1:-}"
LINUX_ARCH="$(release_gpui_linux_arch)"
OUTPUT="${2:-$(release_gpui_default_output "$REPO_ROOT" "$VERSION" "linux-appimage-$LINUX_ARCH")}"
release_gpui_require_version "$VERSION"
release_gpui_require_command mksquashfs
release_gpui_require_command unsquashfs
release_gpui_require_command tar
release_gpui_require_command curl
release_gpui_prepare_output "$REPO_ROOT" "$OUTPUT"

# CDXC:Release 2026-10-26 WHY:
# The AppImage runtime (the ELF header that mounts the image) is pinned by
# release and digest instead of fetched through appimagetool's "continuous"
# channel, so every release embeds bytes we have checked. It is the static
# type2 runtime: it needs no libfuse on the user's computer, only the kernel's
# FUSE and fusermount; without them it stops with a message naming
# --appimage-extract-and-run, which works because AppRun copies the payload
# into the user's data folder either way.
APPIMAGE_RUNTIME_RELEASE="20251108"
case "$LINUX_ARCH" in
x64)
	APPIMAGE_RUNTIME_NAME="runtime-x86_64"
	APPIMAGE_RUNTIME_SHA256="2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d"
	;;
arm64)
	APPIMAGE_RUNTIME_NAME="runtime-aarch64"
	APPIMAGE_RUNTIME_SHA256="00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444"
	;;
esac

# CDXC:Release 2026-10-26: shared staged root, see linux-deb.sh. The AppImage
# wraps opt/ghostex and the desktop entry and icon; usr/bin is left out because
# the app installs its own `ghostex` and `gx` commands into ~/.local/bin on
# first start (gpui_auto_install_ghostex_cli_wrappers), pointing at the copy
# AppRun keeps in the user's data folder.
PACKAGE_ROOT="${GHOSTEX_LINUX_PACKAGE_ROOT:-$REPO_ROOT/build/release-gpui/linux-appimage-package-root}"
if [[ "${GHOSTEX_LINUX_PACKAGE_ROOT_READY:-0}" != "1" ]]; then
	"$SCRIPT_DIR/linux-stage.sh" "$VERSION" "$PACKAGE_ROOT"
fi
for required in opt/ghostex/Ghostex usr/share/applications/ghostex.desktop usr/share/icons/hicolor/256x256/apps/ghostex.png; do
	[[ -e "$PACKAGE_ROOT/$required" ]] || {
		echo "Linux package root is not staged: $PACKAGE_ROOT/$required is missing" >&2
		exit 1
	}
done

WORK="$REPO_ROOT/build/release-gpui/linux-appimage-work-$LINUX_ARCH"
rm -rf "$WORK"
mkdir -p "$WORK"
trap 'rm -rf "$WORK"' EXIT
APPDIR="$WORK/Ghostex.AppDir"
mkdir -p "$APPDIR/opt" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/256x256/apps"
cp -a "$PACKAGE_ROOT/opt/ghostex" "$APPDIR/opt/ghostex"
cp "$PACKAGE_ROOT/usr/share/icons/hicolor/256x256/apps/ghostex.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/ghostex.png"
cp "$PACKAGE_ROOT/usr/share/icons/hicolor/256x256/apps/ghostex.png" "$APPDIR/ghostex.png"
ln -s ghostex.png "$APPDIR/.DirIcon"
# The packaged entry runs /usr/bin/ghostex, which an AppImage does not have.
# Desktop integration tools (AppImageLauncher, Gear Lever, appimaged) replace
# Exec with the AppImage's own path when they install this entry.
sed -e 's|^Exec=.*|Exec=ghostex %U|' \
	"$PACKAGE_ROOT/usr/share/applications/ghostex.desktop" >"$APPDIR/ghostex.desktop"
printf 'X-AppImage-Version=%s\n' "$VERSION" >>"$APPDIR/ghostex.desktop"
cp "$APPDIR/ghostex.desktop" "$APPDIR/usr/share/applications/ghostex.desktop"
install -m 755 "$REPO_ROOT/tooling/packaging/appimage/AppRun" "$APPDIR/AppRun"

# The payload id AppRun compares with the copy in the user's data folder: the
# version plus a digest of the payload, so a rebuilt image of the same version
# still replaces a stale copy.
PAYLOAD_DIGEST="$(
	cd "$APPDIR/opt/ghostex"
	tar --sort=name --mtime=@946684800 --owner=0 --group=0 --numeric-owner -cf - . | sha256sum | awk '{ print $1 }'
)"
printf '%s-%s\n' "$VERSION" "${PAYLOAD_DIGEST:0:16}" >"$APPDIR/.ghostex-appimage"

RUNTIME="$WORK/$APPIMAGE_RUNTIME_NAME"
if [[ -n "${GHOSTEX_APPIMAGE_RUNTIME_FILE:-}" ]]; then
	cp "$GHOSTEX_APPIMAGE_RUNTIME_FILE" "$RUNTIME"
else
	"$SCRIPT_DIR/retry.sh" 4 5 -- curl -fsSL --output "$RUNTIME" \
		"https://github.com/AppImage/type2-runtime/releases/download/$APPIMAGE_RUNTIME_RELEASE/$APPIMAGE_RUNTIME_NAME"
fi
[[ "$(release_gpui_sha256 "$RUNTIME")" == "$APPIMAGE_RUNTIME_SHA256" ]] || {
	echo "AppImage runtime $APPIMAGE_RUNTIME_RELEASE/$APPIMAGE_RUNTIME_NAME does not match its pinned sha256" >&2
	exit 1
}

# Deterministic image: root-owned entries, one timestamp, sorted by mksquashfs.
# zstd is what the type2 runtime reads fastest; 1 MiB blocks hold the payload
# close to the tarball's size.
SQUASHFS="$WORK/payload.squashfs"
mksquashfs "$APPDIR" "$SQUASHFS" \
	-root-owned -noappend -no-xattrs -no-progress -quiet \
	-comp zstd -Xcompression-level 19 -b 1M \
	-mkfs-time 946684800 -all-time 946684800
APPIMAGE="$OUTPUT/ghostex-${VERSION}-linux-${LINUX_ARCH}.AppImage"
cat "$RUNTIME" "$SQUASHFS" >"$APPIMAGE"
chmod 755 "$APPIMAGE"

# Read the image back the way the runtime does: the squashfs starts right after
# the runtime, and AppRun, the payload id, the entry and the icon are inside.
OFFSET="$(stat -c '%s' "$RUNTIME")"
MEMBERS="$(unsquashfs -o "$OFFSET" -lc "$APPIMAGE" | sed 's|^squashfs-root||')"
for required in /AppRun /.ghostex-appimage /ghostex.desktop /ghostex.png /.DirIcon \
	/opt/ghostex/Ghostex /opt/ghostex/ghostex-gpui-runtime /opt/ghostex/gxserver/bin/gxserver \
	/opt/ghostex/gxserver/bin/ghostex /opt/ghostex/resources/on-demand-resources.json \
	/opt/ghostex/gxserver/skills/ghostex-cli/SKILL.md; do
	grep -qxF "$required" <<<"$MEMBERS" || {
		echo "AppImage is missing $required" >&2
		exit 1
	}
done
if grep -q '^/opt/ghostex/libcef\.so$' <<<"$MEMBERS"; then
	echo "AppImage bundles libcef.so; CEF must stay an on-demand component" >&2
	exit 1
fi

release_gpui_write_manifest "$OUTPUT" "linux-appimage-$LINUX_ARCH" "$VERSION" "$APPIMAGE"
printf 'Built Linux %s AppImage in %s\n' "$LINUX_ARCH" "$OUTPUT"
