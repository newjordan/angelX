# Labyrinth Exploration engine in Angel

Selected MIT resources from `nasqret/labyrinth-exploration`, pinned by
`UPSTREAM.json`. The GitHub fork is `newjordan/labyrinth-exploration`.

`lab.py` and `dashboard.html` are embedded in the cockpit and copied into a
workspace by `/labyrinth init`. The engine adaptation reads immutable Angel
observation fragments alongside curated knowledge and its event log, and escapes
research strings in the dashboard's embedded data. It retains upstream's
standard-library-only engine and schema-1 formats. The dashboard and schema are
otherwise retained verbatim.

Reading the map never runs workspace Python or makes model calls. Model calls
happen only in a campaign someone starts (`angel --labyrinth campaign`, or the
`labyrinth_campaign` and `loop_research` tools): literature, attack, independent referee and writer
roles run as native angelX turns, and their checks run in the sealed sandbox. See
[the guide](../../../docs/LABYRINTH.md) for scope and commands and
[the retained license](../../../third-party/labyrinth-exploration-LICENSE.txt).
