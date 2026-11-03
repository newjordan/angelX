#!/usr/bin/env bash
# Print one angelY seat's cockpit screen as plain text (graphics/braille stripped).
kitty @ --to unix:/tmp/angelY.sock get-text --match "env:ANGELY_SEAT=${1:?seat}" 2>&1 | python3 -c 'import sys,re
for l in sys.stdin:
    s=re.sub(r"[\U000F0000-\U0010FFFF̀-ͯ҃-҉֑-ׇؐ-ؚ܀-ݏ᷀-᷿⠀-⣿]","",l).rstrip()
    if s.strip(" │├┤─╭╮╰╯┴┬"): print(s[:200])'
