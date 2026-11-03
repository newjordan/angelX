"""Reproducible sample calls. Local Qwen only; every request is logged."""
from pathlib import Path
import tempfile
from backend import generate

SOURCES = Path(__file__).resolve().parent / 'sources'
BRIEFS = {
    'marsh-watchman': ('body', 'Hunched marsh watchman, reed coat with dark face void, two free arms, split legs and planted boots; one dull ochre eye. No weapon, no scenery.'),
    'cinder-knave': ('body', 'Cinder knave monster, lean goblin humanoid with a broad wedge head and long pointed nose, grey padded vest, exposed bent elbows clearly separated from torso, short separated legs and large boots. Small dull ochre belt buckle. No cloak, no weapon, no background details.'),
    'chain-flail': ('weapon', 'Medieval flail alone, thick short dark grey grip at bottom, a clearly visible linked chain curving to the right and a large spiked iron ball at the top. Three bold spikes. Tiny ochre pommel. Simple chunky shapes.'),
    'hearth-greatsword': ('weapon', 'Heavy medieval greatsword alone, upright broad long grey steel blade with angled tip and dark fuller, wide cruciform crossguard, dark leather grip at bottom, tiny ochre pommel. Big readable blade.'),
    'ash-spear': ('weapon', 'Medieval spear alone, upright long ash-grey straight shaft with broad leaf shaped steel point at top, small ochre binding below spearhead, grip near bottom. Bold simple silhouette.'),
    'rivet-cuirass': ('worn', 'Empty steel torso cuirass with two rounded pauldrons, wide angular breastplate with dark inset centre, three small dull ochre rivets and a waist band. No body inside. Horizontal short broad armour only.'),
    'lamellar-vest': ('worn', 'Empty medieval lamellar torso armour, wide short rectangular steel vest with two shoulder straps, three bold horizontal overlapping grey plate rows, small dull ochre lacing on one side. No person, head, arms or legs.'),
    'iron-hit-sparks': ('vfx', 'A compact radial burst of five short chunky muted steel grey impact sparks, centre dark and small, two tiny ochre fragments, isolated in empty space. No white glow, no fireball, no gradients.'),
    'tavern-cask': ('prop', 'A squat medieval tavern ale cask, bulging muted grey timber staves, two thick grey iron hoops, small dull ochre tap on lower right and round bung near top. Front side view, no ground shadow.'),
}

if __name__ == '__main__':
    for i, (ident, (kind, brief)) in enumerate(BRIEFS.items(), 3902):
        output = SOURCES / (ident + '.png')
        if output.exists():
            continue
        print(ident, generate(brief, kind, output, str(Path(tempfile.gettempdir()) / 'asset-factory' / 'generation-calls.jsonl'), i), flush=True)
