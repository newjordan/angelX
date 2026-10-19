# Dungeon HUD chrome sheet notes

Reading order (4 columns x 3 rows):

1. READY action plate — square navy face, bright brass rails and four iron corner studs.
2. COOLDOWN action plate — same square footprint, dark iron rails and muted brass wear.
3. DISABLED action plate — near-black navy face and subdued iron rim.
4. PRESSED action plate — brighter brass rails and visibly inset dark center.
5. Health bar frame — long narrow navy trough with brass top/bottom and iron end caps.
6. Stamina/mana bar frame — shorter trough with matching but distinct end caps.
7. Boss health frame — wide ornate iron/brass frame with centered upper name notch.
8. Seat nameplate pip — compact iron plate with a red jewel socket.
9. Floor/room indicator — compact square navy plate with iron studs and brass directional points.
10. Minimap frame — open square frame with ornate metal corners and brass details.
11. Status icon plate — round navy medallion with iron sides and brass accents.
12. Toast/message plate — wide straight-sided frame with a plain empty navy middle.

Weaknesses: generated source was 1254x1254 instead of 1536x1536; the final accepted PNG was padded with transparent pixels to the requested canvas size. The first two generations placed adjacent frames too close for the crop detector; the final regeneration yielded 12 unmerged crops. The pressed/ready visual difference is present but modest, and individual pieces are larger than their in-game native pixel sizes, so engine downsampling should be reviewed in integration.

Possible next revision: strengthen the pressed plate's inward compression cue while keeping the interior empty, and reduce detail contrast slightly for 24-32 px display.
