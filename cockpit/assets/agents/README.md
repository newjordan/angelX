# Agent portrait assets

The default portraits use the eight-pose sheets in [helms/](helms/PROVENANCE.md).
[helm.rs](../../src/ui/helm.rs) crops the pose cells;
[agent_panel_view.rs](../../src/ui/draw/agent_panel_view.rs) selects poses from live
agent state. With portrait states disabled, [agent_profile.rs](../../src/ui/agent_panel/profile.rs)
selects the champion effort pairs.

The champion, cyberknight and earlier neutral/active PNGs are retained original
artwork. Their filenames identify separate portrait generations; they are not
all selected by the current renderer. The helms directory retains its prompts
and source manifest.
