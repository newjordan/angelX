#!/bin/sh
# Build a game's browser bundle: crates/web/build.sh <game-crate-dir> [game]
#
#   crates/web/build.sh web/testgame testgame
#
# <game-crate-dir> is relative to crates/ and holds a cdylib that invokes
# angel_web::export_client!. [game] names the bundle (default: the crate
# directory's name). Writes crates/web/dist/<game>/:
#
#   index.html     the shell page, {{asset:NAME}} left for the server to fill
#                  with /a/<hash>.<ext>; the sounds manifest is inline in it
#   *.js, *.css    the shell
#   <game>.wasm    the game's client
#   <sound files>  from <game-crate-dir>/sounds/, if there is one, and from
#                  the folders <game-crate-dir>/sounds.list names (a name
#                  prefix, "-" for none, then a folder, a line each)
#
# and prints each file's size and its gzip size (what a browser downloads),
# then the first load (the page, the shell and the wasm) apart from the
# sounds, which the shell fetches only when a sound is first asked for.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
crates=$(cd "$here/.." && pwd)
crate=${1:?usage: build.sh <game-crate-dir> [game]}
crate_dir=$(cd "$crates/$crate" && pwd)
game=${2:-$(basename "$crate_dir")}

package=$(sed -n 's/^name *= *"\(.*\)"/\1/p' "$crate_dir/Cargo.toml" | head -n 1)
lib=$(printf '%s' "$package" | tr - _)
target_dir=${CARGO_TARGET_DIR:-$crates/target}

(cd "$crates" && cargo build -p "$package" --profile web --target wasm32-unknown-unknown)

out="$here/dist/$game"
rm -rf "$out"
mkdir -p "$out"
cp "$target_dir/wasm32-unknown-unknown/web/$lib.wasm" "$out/$game.wasm"
cp "$here/shell/"*.js "$here/shell/shell.css" "$out/"

# The sounds manifest: {"name": "{{asset:name.ext}}", ...}, inline in the
# page so the server's asset names reach it.
sounds=""
sound_files=""
# add_sounds <folder> <prefix>: every file in the folder, as <prefix><file>.
add_sounds() {
  for f in "$1"/*; do
    [ -f "$f" ] || continue
    file="$2$(basename "$f")"
    case "$file" in *[!A-Za-z0-9._-]*) echo "skipping $file: names are letters, digits, . _ -" >&2; continue ;; esac
    cp "$f" "$out/$file"
    sounds="$sounds${sounds:+,}\"${file%.*}\":\"{{asset:$file}}\""
    sound_files="$sound_files $file"
  done
}
if [ -d "$crate_dir/sounds" ]; then
  add_sounds "$crate_dir/sounds" ""
fi
if [ -f "$crate_dir/sounds.list" ]; then
  while read -r prefix folder; do
    case "$prefix" in ''|'#'*) continue ;; esac
    [ "$prefix" = "-" ] && prefix=""
    [ -d "$crate_dir/$folder" ] || { echo "sounds.list: no folder $folder" >&2; exit 1; }
    add_sounds "$crate_dir/$folder" "$prefix"
  done < "$crate_dir/sounds.list"
fi
sounds="{$sounds}"

title=$(printf '%s' "$game" | awk '{ print toupper(substr($0, 1, 1)) substr($0, 2) }')
sed -e "s|@@GAME@@|$game|g" -e "s|@@TITLE@@|$title|g" -e "s|@@SOUNDS@@|$sounds|" \
  "$here/shell/index.html" > "$out/index.html"

total=0
total_gz=0
first=0
first_gz=0
for f in "$out"/*; do
  size=$(wc -c < "$f")
  gz=$(gzip -9 -c "$f" | wc -c)
  total=$((total + size))
  total_gz=$((total_gz + gz))
  case " $sound_files " in
    *" $(basename "$f") "*) ;;
    *)
      first=$((first + size))
      first_gz=$((first_gz + gz))
      printf '%-24s %9d bytes %9d gzip\n' "$(basename "$f")" "$size" "$gz"
      ;;
  esac
done
count=$(printf '%s\n' $sound_files | grep -c . || true)
printf '%-24s %9d bytes %9d gzip\n' "first load" "$first" "$first_gz"
printf '%-24s %9d bytes %9d gzip   (%d files, fetched when first played)\n' "sounds" "$((total - first))" "$((total_gz - first_gz))" "$count"
printf '%-24s %9d bytes %9d gzip\n' "total" "$total" "$total_gz"
echo "bundle: $out"
