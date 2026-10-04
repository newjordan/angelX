# The Delve: who is down there

The realm is part old Britain, part something new: a kingdom of knights in a
world that is still being computed. Below the town are the Crypt, the Mines
and Dragon Keep. Below those is the Unknown.

There are no villains here. There are guardians, and there are things not met
yet.

## The quest

Every knight in the Delve is on the Grail quest. Old Blaise tells it the way
Percival learned it:

- **Love life, and help where you can.** A hand to a fallen friend weighs more
  than any treasure carried up.
- **Every day is Sisyphean.** The stone rolls back down every night. Percival
  laughed when he understood that. Then he pushed it again, and loved the hill.
- **The Grail is life itself, given to us through time.** It is not kept at the
  bottom of the dark; it is in your hands the whole way down.
- **To find the Grail is to take up its search.** To seek. To be.

The game keeps that shape. Runs end, spoils are lost when the party falls,
and the stone is at the bottom again tomorrow. What the party carries out
alive becomes part of the world. After the first dragon falls, Blaise asks for
one more wish: **The Grail Chapel**, its door left open.

## The cast

| Who | Is | Speaks when |
|---|---|---|
| **The Herald** | The arena's voice: the operator's calibrated announcer | Gates seal, rooms clear, slay streaks, guardians, victory, wipes |
| **Old Blaise** | The hermit who wrote down Merlin's tales, and walked with Percival | A run begins, the party goes deeper, a knight is revived, the dragon, the end |
| **Wren** | A lantern-bearer who draws the map as you go, and is sure the Grail is a cup | Discoveries, rare cards, bonds, the treasury, being worried about you |
| **Tobbin** | The smith. He made most of what you carry and has opinions about all of it | Weapon cards and some others |
| **The Shoggoth** | The great Unknown: many-voiced, vast and curious. Not an enemy, only what you have not met yet | Dragon Keep, the deepest floor, the end of a run, a new shape in the world |
| **The Waxen Warden** | The Crypt's guardian, designed by Muse. It keeps the last candles lit | Its rise, its rage, its fall |
| **Cinderjaw** | The Mines' guardian, designed by Muse: a furnace golem still chewing the first fire | Its rise, its rage, its fall |

## Adding lines

The script is `cockpit/assets/dungeon/voices/chorus.txt`, one line per row:
`who | cue | id | words`. Anyone can add a line; it shows as a subtitle at
once. With `ELEVENLABS_API_KEY` set, `voice.py` records it in that
character's voice.

A cue can be specific (`card_arm:crossbow`, `boss_rise:cinderjaw`,
`wish_raised:grail-chapel`) or general (`card_arm`). Specific lines win.
Several speakers on one cue take turns. **V** mutes the voices in the cockpit;
`/dungeon voices on|off` does the same.
