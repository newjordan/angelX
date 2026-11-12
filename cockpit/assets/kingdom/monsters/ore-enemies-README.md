# Mineral field enemies

`ore-enemies-atlas.png` contains four distinct ordinary encounter enemies: Iron Ore Sentinel, Coal Blast Crawler, Crystal Prism Stalker, and Ember Cinderhound. Their materials foreshadow iron, coal, moon crystal, and living ember deposits. Each design also communicates its native attack: shield bash, planted explosive keg, reflected ranged shots, or a fiery charge.

`ore-enemies-windup-atlas.png` contains the matching anticipation poses. The sentinel raises its hammer, the crawler presents a burning keg, the stalker opens its prism arms and chest, and the hound compresses into a low crouch. Native combat state decides when these pictures appear; the art does not change encounter rules.

The two JSON manifests retain complete built-in imagegen prompts, selected source paths, SHA-256 checksums, exact crop rectangles, and visible alpha bounds. The resting atlas's lower column split is asymmetric to preserve the hound's paws. The windup atlas was revised to provide broad empty gutters around the prism's outstretched arms. Its final four cells are exactly 627 × 627 pixels.

Normalize each pose by its alpha-trimmed silhouette when preparing runtime sprites. The full-resolution pictures have different amounts of transparent cell padding, so raw source-pixel scale must not be shared between sheets. Intended field size is approximately 36 pixels; the hound and crawler become shorter during their crouched windups. Preserve bottom-centre ground anchors. A higher-resolution copy of the same source can serve forecast portraits.

The implemented renderer uses 36-pixel resting banks, a 44-pixel sentinel windup, a 40-pixel prism windup, and 36-pixel crawler/hound windups. The larger raised-hammer and open-arm bounds preserve torso size. The isolated paired-pose review compiles this renderer with synthetic states; it checks appearance and anchors, not encounter timing. Full-engine encounter captures and their fixture limits are documented in [the screenshot index](https://github.com/newjordan/angelX/blob/main/docs/images/kingdom-works/README.md).

Matching resource illustrations are in `../terrain/ore-resources-atlas.png`: ordinary coal stays black, moon crystal uses the stalker's cyan/violet facets, iron uses the sentinel's rusty metallic planes, and living ember uses the hound's molten core. This lets the forecast and the enemy reinforce the same resource identity.
