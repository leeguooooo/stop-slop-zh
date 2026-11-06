#!/bin/sh
# stop-slop 安装脚本：从 GitHub Release 下载预编译二进制。
#   curl -fsSL https://raw.githubusercontent.com/leeguooooo/stop-slop-zh/main/install.sh | sh
# 环境变量：
#   STOP_SLOP_VERSION      指定版本，如 v1.0.0（默认最新）
#   STOP_SLOP_INSTALL_DIR  安装目录（默认 $HOME/.local/bin）
set -eu

REPO="leeguooooo/stop-slop-zh"
BIN="stop-slop"
INSTALL_DIR="${STOP_SLOP_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${STOP_SLOP_VERSION:-}"

say() { printf '%s\n' "$*" >&2; }
die() { say "stop-slop 安装失败：$*"; exit 1; }

os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux) os_part="unknown-linux-gnu" ;;
  *) die "不支持的系统 ${os}（Windows 请到 https://github.com/$REPO/releases 下载 zip）" ;;
esac
case "$arch" in
  arm64 | aarch64) arch_part="aarch64" ;;
  x86_64 | amd64) arch_part="x86_64" ;;
  *) die "不支持的架构 $arch" ;;
esac
target="${arch_part}-${os_part}"
asset="${BIN}-${target}.tar.gz"

if [ -n "$VERSION" ]; then
  base="https://github.com/$REPO/releases/download/$VERSION"
else
  base="https://github.com/$REPO/releases/latest/download"
fi

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q "$1" -O "$2"; }
else
  die "需要 curl 或 wget"
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

say "下载 $base/$asset"
fetch "$base/$asset" "$tmp/$asset" || die "下载 $asset 失败"
fetch "$base/$asset.sha256" "$tmp/$asset.sha256" || die "下载校验文件失败"

expected="$(awk '{print $1}' "$tmp/$asset.sha256")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$asset" | awk '{print $1}')"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$tmp/$asset" | awk '{print $1}')"
else
  die "需要 sha256sum 或 shasum 校验下载文件"
fi
[ "$expected" = "$actual" ] || die "sha256 不匹配（期望 ${expected}，实际 ${actual}）"

tar -xzf "$tmp/$asset" -C "$tmp"
[ -f "$tmp/$BIN" ] || die "压缩包里没有 $BIN"
mkdir -p "$INSTALL_DIR"
mv "$tmp/$BIN" "$INSTALL_DIR/$BIN"
chmod +x "$INSTALL_DIR/$BIN"

say "已安装：$INSTALL_DIR/${BIN}（$("$INSTALL_DIR/$BIN" --version 2>/dev/null || echo "$BIN")）"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) say "提示：$INSTALL_DIR 不在 PATH 里，把这行加进 shell 配置：export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac
