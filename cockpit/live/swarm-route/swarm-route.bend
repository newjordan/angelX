import Base

# One problem's routing decision. Rust does the text work (trim, lowercase,
# word split, phrase windows, prefix tests) and hands this record the resulting
# booleans and counts; the combination below is the whole heuristic.

type Features is Data:
  Feat{blank: Bool, stripped_blank: Bool, bytes: U32, chars: U32, open_hits: U32, enum_lines: U32, trivial: Bool, moa_exact: Bool, moa_prefix: Bool, open_phrase: Bool, open_word: Bool, action_word: Bool, live_phrase: Bool, live_word: Bool}

type Decisions is Data:
  Route{tight: Bool, moa: Bool, synthesis: Bool, deliberation: Bool, research: Bool, hard: Bool}

# `tight` is the chit-chat fast path, `moa` an explicit leading summon, the
# three wants are the fan-out gates, and `hard` selects multi-wave difficulty.
def wants(f: Features) -> Decisions:
  match f:
    case Feat{+blank, +stripped_blank, +bytes, +chars, +open_hits, +enum_lines, +trivial, +moa_exact, +moa_prefix, +open_phrase, +open_word, +action_word, +live_phrase, +live_word}:
      Route{
        Bool.or(stripped_blank, trivial),
        Bool.or(moa_exact, moa_prefix),
        Bool.and(Bool.not(blank), Bool.or(U32.is_gt(bytes, 600), Bool.or(open_phrase, open_word))),
        Bool.and(Bool.not(blank), Bool.or(U32.is_gt(bytes, 400), Bool.or(Bool.or(open_phrase, open_word), action_word))),
        Bool.and(Bool.not(blank), Bool.or(live_phrase, live_word)),
        Bool.or(U32.is_gt(chars, 800), Bool.or(U32.is_ge(open_hits, 2), U32.is_ge(enum_lines, 3)))
      }
