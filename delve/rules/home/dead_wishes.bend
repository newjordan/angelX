# crates/delve/src/rules/dead_wishes.rs — a summoner's wishes for his dead.
# (`named` is with the scroll's policy, scrolls.bend.)

def dead_wishes.COLOURS() -> List<&2, String>:
  ["red", "crimson", "scarlet", "ruby", "blood", "pink", "rose", "rosy", "magenta", "fuchsia", "purple", "violet", "lilac", "lavender", "blue", "azure", "navy", "cyan", "teal", "turquoise", "green", "emerald", "jade", "lime", "yellow", "gold", "golden", "amber", "orange", "copper", "bronze", "brown", "black", "white", "silver", "grey", "gray", "ivory", "pale", "dark", "rainbow", "neon", "glowing", "colour"]

# Whether a name (a card's, "Bright Pink") names a colour.
def dead_wishes.names_colour(+name: String) -> Bool:
  dead_wishes.names_colour.any(cards.words_of(home.util.lower(name)))

def dead_wishes.names_colour.any(+ws: List<&2, String>) -> Bool:
  match ws:
    case Nil{}:
      False{}
    case Con{+w, +t}:
      Bool.or(Bool.or(home.util.has_name(dead_wishes.COLOURS(), w), home.util.has_name(["color", "coloured", "colored"], w)), dead_wishes.names_colour.any(t))

# Whether held card `card`'s worn look colours a body rather than sits on it.
def dead_wishes.colours_body(+card: J.Val) -> Bool:
  dead_wishes.colours_body.of(J.get(J.get(card, "looks"), "worn"), J.str(J.get(card, "name")))

def dead_wishes.colours_body.of(+worn: J.Val, +name: String) -> Bool:
  match worn:
    case J.JNull{}:
      False{}
    case _:
      Bool.and(Bool.or(J.is_null(J.get(worn, "at")), Bool.or(String.eq(J.str(J.get(worn, "at")), "body"), String.eq(J.str(J.get(worn, "at")), "whole"))), dead_wishes.names_colour(name))

# The wish a ladder's card is a tier of (`wish-<id>-<tier>`), or "".
def dead_wishes.wish_of(+card: String) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, String.starts_with(card, "wish-"),
    u => dead_wishes.wish_of.rest(String.drop(card, 5n)),
    u => None{})

def dead_wishes.wish_of.rest(+rest: String) -> Maybe<&2, String>:
  dead_wishes.wish_of.split(String.reverse(rest))

# rsplit_once('-') on the reversed text: (tier reversed, id reversed)
def dead_wishes.wish_of.split(+rev: String) -> Maybe<&2, String>:
  dead_wishes.wish_of.cut(rev, "")

def dead_wishes.wish_of.cut(+rev: String, +tier: String) -> Maybe<&2, String>:
  match rev:
    case SNil{}:
      None{}
    case SCon{+h, +t}:
      home.util.ite(Maybe<&2, String>, U32.is_eq(Char.to_u32(h), 45),
        u => dead_wishes.wish_of.fin(String.reverse(t), tier),
        u => dead_wishes.wish_of.cut(t, String.append(tier, SCon{h, SNil{}})))

def dead_wishes.wish_of.fin(+id: String, +tier: String) -> Maybe<&2, String>:
  home.util.ite(Maybe<&2, String>, Bool.and(Bool.not(String.is_empty(id)), Bool.and(Bool.not(String.is_empty(tier)), home.util.all_in(tier, [48, 57, 48, 57], ""))),
    u => Some{id},
    u => None{})
