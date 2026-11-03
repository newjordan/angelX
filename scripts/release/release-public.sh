#!/usr/bin/env bash
# Cut a release for the public repo as one commit holding dev's exact tree; the working history stays private.
#   scripts/release/release-public.sh 0.1.7 "angelX 0.1.7: <one line>"
#   scripts/release/release-public.sh - "angelX 0.1.6: benchmarks"         (an update between versions: no tag)
# Remotes: origin = newjordan/angelX-private (all work), public = newjordan/angelX (releases only).
# It builds the commit on public/main, moves local main and tags it, then merges the release back into dev
# (same tree) so the next release stacks on this one. Nothing is pushed; it prints the push commands.
set -euo pipefail
ver=${1:?version, e.g. 0.1.7, or - for no tag} msg=${2:?release message} src=dev
cd "$(git rev-parse --show-toplevel)"
[ -z "$(git status --porcelain)" ] || { echo "working tree not clean" >&2; exit 1; }
[ "$ver" = - ] || ! git rev-parse -q --verify "refs/tags/v$ver" >/dev/null || { echo "tag v$ver already exists" >&2; exit 1; }
git fetch -q public
git merge-base --is-ancestor public/main "$src" \
  || { echo "public/main has commits $src lacks; merge public/main into $src first" >&2; exit 1; }
[ "$(git rev-parse main)" = "$(git rev-parse public/main)" ] \
  || { echo "local main differs from public/main; run: git branch -f main public/main" >&2; exit 1; }
# Nothing not safe for work ships: the game's content (cards, wishes, art prompts, the play-test fixtures)
# carries none of the words its checker refuses (NOT_FOR_WORK in crates/delve/src/rules/cards.rs, rot13 there).
nsfw=$(git show "$src:crates/delve/src/rules/cards.rs" | sed -n '/^const NOT_FOR_WORK/,/^];/p' | grep -o '"[a-z]*"' | tr -d '"' | tr 'a-z' 'n-za-m' | paste -sd'|')
[ -n "$nsfw" ] || { echo "the NOT_FOR_WORK list is missing from crates/delve/src/rules/cards.rs" >&2; exit 1; }
if git grep -I -n -i -w -E "$nsfw" "$src" -- cockpit/assets/dungeon cockpit/assets/realm crates/delve/assets crates/delve/tests/rules/fixtures; then
  echo "not safe for work content above: take it out of $src before a release" >&2; exit 1
fi

# The public tree is dev's tree without release/public-exclude.txt's paths (operator and
# agent working files, private planning); dev keeps them. The list is read as committed.
tree=$(git rev-parse "$src^{tree}")
if git cat-file -e "$src:release/public-exclude.txt" 2>/dev/null; then
  idx=$(mktemp); trap 'rm -f "$idx"' EXIT
  GIT_INDEX_FILE=$idx git read-tree "$src^{tree}"
  git show "$src:release/public-exclude.txt" | sed -e 's/#.*//' -e 's/[[:space:]]*$//' | grep -v '^$' \
    | while IFS= read -r spec; do GIT_INDEX_FILE=$idx git rm -r -q --cached --ignore-unmatch -- "$spec" >/dev/null; done
  tree=$(GIT_INDEX_FILE=$idx git write-tree)
  echo "public tree leaves out $(git diff --name-only "$src^{tree}" "$tree" | wc -l) private file(s) (release/public-exclude.txt)"
fi
rel=$(git commit-tree "$tree" -p public/main -m "$msg")
git branch -f main "$rel"
[ "$ver" = - ] || git tag -a "v$ver" "$rel" -m "$msg"
old=$(git rev-parse "$src")
git update-ref "refs/heads/$src" "$(git commit-tree "$src^{tree}" -p "$old" -p "$rel" -m "merge: $msg (as released on public main)")" "$old"

tags=; [ "$ver" = - ] || tags=" v$ver"
echo "release${tags:- (no tag)} = $(git rev-parse --short "$rel") on public/main $(git rev-parse --short public/main)"
echo "publish:  git push public main$tags && git push origin main $src$tags"
