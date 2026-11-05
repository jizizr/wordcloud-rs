#!/bin/sh
# Downloads the Chinese fonts used by the gallery examples into
# examples/data/fonts/. All of them are licensed under the SIL Open Font License.
set -eu
dir="$(dirname "$0")/data/fonts"
mkdir -p "$dir"
gf=https://raw.githubusercontent.com/google/fonts/main/ofl
get() {
    [ -f "$dir/$1" ] || { echo "downloading $1"; curl -fsSL -o "$dir/$1" "$2"; }
}
get LXGWWenKai-Medium.ttf https://raw.githubusercontent.com/lxgw/LxgwWenKai/main/fonts/TTF/LXGWWenKai-Medium.ttf
get ZCOOLKuaiLe-Regular.ttf "$gf/zcoolkuaile/ZCOOLKuaiLe-Regular.ttf"
get MaShanZheng-Regular.ttf "$gf/mashanzheng/MaShanZheng-Regular.ttf"
get NotoSerifSC.ttf "$gf/notoserifsc/NotoSerifSC%5Bwght%5D.ttf"
