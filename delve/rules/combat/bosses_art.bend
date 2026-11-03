# GENERATED from rules/bosses.rs (Art::for_id) -- do not edit.

# bosses::Art, the ids a guardian's art is known by (Art::for_id).

def bosses.Art.known(+id: String) -> Bool:
  bosses.Art.known.n0(id)

def bosses.Art.known.n0(+id: String) -> Bool:
  bosses.Art.known.k0(String.eq(id, "cinderjaw"), id)

def bosses.Art.known.k0(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n1(id)

def bosses.Art.known.n1(+id: String) -> Bool:
  bosses.Art.known.k1(String.eq(id, "the-index"), id)

def bosses.Art.known.k1(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n2(id)

def bosses.Art.known.n2(+id: String) -> Bool:
  bosses.Art.known.k2(String.eq(id, "mother-of-spores"), id)

def bosses.Art.known.k2(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n3(id)

def bosses.Art.known.n3(+id: String) -> Bool:
  bosses.Art.known.k3(String.eq(id, "waxen-warden"), id)

def bosses.Art.known.k3(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n4(id)

def bosses.Art.known.n4(+id: String) -> Bool:
  bosses.Art.known.k4(String.eq(id, "the-bone-choir"), id)

def bosses.Art.known.k4(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n5(id)

def bosses.Art.known.n5(+id: String) -> Bool:
  bosses.Art.known.k5(String.eq(id, "crypt-tithe-marshal"), id)

def bosses.Art.known.k5(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n6(id)

def bosses.Art.known.n6(+id: String) -> Bool:
  bosses.Art.known.k6(String.eq(id, "crypt-tax-keeper"), id)

def bosses.Art.known.k6(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n7(id)

def bosses.Art.known.n7(+id: String) -> Bool:
  bosses.Art.known.k7(String.eq(id, "crypt-relic-liege"), id)

def bosses.Art.known.k7(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n8(id)

def bosses.Art.known.n8(+id: String) -> Bool:
  bosses.Art.known.k8(String.eq(id, "iron-tusk-chief"), id)

def bosses.Art.known.k8(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n9(id)

def bosses.Art.known.n9(+id: String) -> Bool:
  bosses.Art.known.k9(String.eq(id, "red-pick-chief"), id)

def bosses.Art.known.k9(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n10(id)

def bosses.Art.known.n10(+id: String) -> Bool:
  bosses.Art.known.k10(String.eq(id, "the-foreman"), id)

def bosses.Art.known.k10(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n11(id)

def bosses.Art.known.n11(+id: String) -> Bool:
  bosses.Art.known.k11(String.eq(id, "late-fee-leviathan"), id)

def bosses.Art.known.k11(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n12(id)

def bosses.Art.known.n12(+id: String) -> Bool:
  bosses.Art.known.k12(String.eq(id, "archive-bellkeeper"), id)

def bosses.Art.known.k12(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n13(id)

def bosses.Art.known.n13(+id: String) -> Bool:
  bosses.Art.known.k13(String.eq(id, "fungal-rootwarden"), id)

def bosses.Art.known.k13(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n14(id)

def bosses.Art.known.n14(+id: String) -> Bool:
  bosses.Art.known.k14(String.eq(id, "fungal-spore-crown"), id)

def bosses.Art.known.k14(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n15(id)

def bosses.Art.known.n15(+id: String) -> Bool:
  bosses.Art.known.k15(String.eq(id, "grinwick"), id)

def bosses.Art.known.k15(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n16(id)

def bosses.Art.known.n16(+id: String) -> Bool:
  bosses.Art.known.k16(String.eq(id, "hellforge-ash-castellan"), id)

def bosses.Art.known.k16(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n17(id)

def bosses.Art.known.n17(+id: String) -> Bool:
  bosses.Art.known.k17(String.eq(id, "hellforge-chain-keeper"), id)

def bosses.Art.known.k17(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n18(id)

def bosses.Art.known.n18(+id: String) -> Bool:
  bosses.Art.known.k18(String.eq(id, "hellforge-free-captain"), id)

def bosses.Art.known.k18(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n19(id)

def bosses.Art.known.n19(+id: String) -> Bool:
  bosses.Art.known.k19(String.eq(id, "hellforge-fuel-reeve"), id)

def bosses.Art.known.k19(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n20(id)

def bosses.Art.known.n20(+id: String) -> Bool:
  bosses.Art.known.k20(String.eq(id, "hellforge-slag-sovereign"), id)

def bosses.Art.known.k20(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n21(id)

def bosses.Art.known.n21(+id: String) -> Bool:
  bosses.Art.known.k21(String.eq(id, "salvage-claimant"), id)

def bosses.Art.known.k21(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n22(id)

def bosses.Art.known.n22(+id: String) -> Bool:
  bosses.Art.known.k22(String.eq(id, "shoggoth-heart"), id)

def bosses.Art.known.k22(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n23(id)

def bosses.Art.known.n23(+id: String) -> Bool:
  bosses.Art.known.k23(String.eq(id, "the-big-ripper"), id)

def bosses.Art.known.k23(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n24(id)

def bosses.Art.known.n24(+id: String) -> Bool:
  bosses.Art.known.k24(String.eq(id, "unknown-memory-warden"), id)

def bosses.Art.known.k24(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      bosses.Art.known.n25(id)

def bosses.Art.known.n25(+id: String) -> Bool:
  bosses.Art.known.k25(String.eq(id, "unknown-silence-warden"), id)

def bosses.Art.known.k25(hit: Bool, +id: String) -> Bool:
  match hit:
    case True{}:
      True{}
    case False{}:
      False{}

# Art::of_shot(shot).and_then(|art| art.look()): the shot's look if it is a
# known guardian's art, else None.
def bosses.Art.look_of(+look: J.Val) -> J.Val:
  bosses.Art.look_of.go(String.eq(J.str(J.nth(look, 0)), "boss-art"), bosses.Art.known(J.str(J.nth(look, 1))), look)

def bosses.Art.look_of.go(fam: Bool, known: Bool, +look: J.Val) -> J.Val:
  bosses.Art.look_of.pick(Bool.and(fam, known), look)

def bosses.Art.look_of.pick(both: Bool, +look: J.Val) -> J.Val:
  match both:
    case True{}:
      look
    case False{}:
      J.JNull{}

# Art::for_id as the name serde writes the Art by (boss_art on a Ravage).
def bosses.Art.name_of(+id: String) -> String:
  bosses.Art.name_of.n0(id)

def bosses.Art.name_of.n0(+id: String) -> String:
  bosses.Art.name_of.k0(String.eq(id, "cinderjaw"), id)

def bosses.Art.name_of.k0(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "cinderjaw"
    case False{}:
      bosses.Art.name_of.n1(id)

def bosses.Art.name_of.n1(+id: String) -> String:
  bosses.Art.name_of.k1(String.eq(id, "the-index"), id)

def bosses.Art.name_of.k1(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "index"
    case False{}:
      bosses.Art.name_of.n2(id)

def bosses.Art.name_of.n2(+id: String) -> String:
  bosses.Art.name_of.k2(String.eq(id, "mother-of-spores"), id)

def bosses.Art.name_of.k2(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "mother"
    case False{}:
      bosses.Art.name_of.n3(id)

def bosses.Art.name_of.n3(+id: String) -> String:
  bosses.Art.name_of.k3(String.eq(id, "waxen-warden"), id)

def bosses.Art.name_of.k3(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "waxen"
    case False{}:
      bosses.Art.name_of.n4(id)

def bosses.Art.name_of.n4(+id: String) -> String:
  bosses.Art.name_of.k4(String.eq(id, "the-bone-choir"), id)

def bosses.Art.name_of.k4(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "choir"
    case False{}:
      bosses.Art.name_of.n5(id)

def bosses.Art.name_of.n5(+id: String) -> String:
  bosses.Art.name_of.k5(String.eq(id, "crypt-tithe-marshal"), id)

def bosses.Art.name_of.k5(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "marshal"
    case False{}:
      bosses.Art.name_of.n6(id)

def bosses.Art.name_of.n6(+id: String) -> String:
  bosses.Art.name_of.k6(String.eq(id, "crypt-tax-keeper"), id)

def bosses.Art.name_of.k6(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "tax_keeper"
    case False{}:
      bosses.Art.name_of.n7(id)

def bosses.Art.name_of.n7(+id: String) -> String:
  bosses.Art.name_of.k7(String.eq(id, "crypt-relic-liege"), id)

def bosses.Art.name_of.k7(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "relic_liege"
    case False{}:
      bosses.Art.name_of.n8(id)

def bosses.Art.name_of.n8(+id: String) -> String:
  bosses.Art.name_of.k8(String.eq(id, "iron-tusk-chief"), id)

def bosses.Art.name_of.k8(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "iron_tusk"
    case False{}:
      bosses.Art.name_of.n9(id)

def bosses.Art.name_of.n9(+id: String) -> String:
  bosses.Art.name_of.k9(String.eq(id, "red-pick-chief"), id)

def bosses.Art.name_of.k9(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "red_pick"
    case False{}:
      bosses.Art.name_of.n10(id)

def bosses.Art.name_of.n10(+id: String) -> String:
  bosses.Art.name_of.k10(String.eq(id, "the-foreman"), id)

def bosses.Art.name_of.k10(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "foreman"
    case False{}:
      bosses.Art.name_of.n11(id)

def bosses.Art.name_of.n11(+id: String) -> String:
  bosses.Art.name_of.k11(String.eq(id, "late-fee-leviathan"), id)

def bosses.Art.name_of.k11(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "leviathan"
    case False{}:
      bosses.Art.name_of.n12(id)

def bosses.Art.name_of.n12(+id: String) -> String:
  bosses.Art.name_of.k12(String.eq(id, "archive-bellkeeper"), id)

def bosses.Art.name_of.k12(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "bellkeeper"
    case False{}:
      bosses.Art.name_of.n13(id)

def bosses.Art.name_of.n13(+id: String) -> String:
  bosses.Art.name_of.k13(String.eq(id, "fungal-rootwarden"), id)

def bosses.Art.name_of.k13(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "rootwarden"
    case False{}:
      bosses.Art.name_of.n14(id)

def bosses.Art.name_of.n14(+id: String) -> String:
  bosses.Art.name_of.k14(String.eq(id, "fungal-spore-crown"), id)

def bosses.Art.name_of.k14(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "spore_crown"
    case False{}:
      bosses.Art.name_of.n15(id)

def bosses.Art.name_of.n15(+id: String) -> String:
  bosses.Art.name_of.k15(String.eq(id, "grinwick"), id)

def bosses.Art.name_of.k15(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "grinwick"
    case False{}:
      bosses.Art.name_of.n16(id)

def bosses.Art.name_of.n16(+id: String) -> String:
  bosses.Art.name_of.k16(String.eq(id, "hellforge-ash-castellan"), id)

def bosses.Art.name_of.k16(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "ash_castellan"
    case False{}:
      bosses.Art.name_of.n17(id)

def bosses.Art.name_of.n17(+id: String) -> String:
  bosses.Art.name_of.k17(String.eq(id, "hellforge-chain-keeper"), id)

def bosses.Art.name_of.k17(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "chain_keeper"
    case False{}:
      bosses.Art.name_of.n18(id)

def bosses.Art.name_of.n18(+id: String) -> String:
  bosses.Art.name_of.k18(String.eq(id, "hellforge-free-captain"), id)

def bosses.Art.name_of.k18(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "free_captain"
    case False{}:
      bosses.Art.name_of.n19(id)

def bosses.Art.name_of.n19(+id: String) -> String:
  bosses.Art.name_of.k19(String.eq(id, "hellforge-fuel-reeve"), id)

def bosses.Art.name_of.k19(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "fuel_reeve"
    case False{}:
      bosses.Art.name_of.n20(id)

def bosses.Art.name_of.n20(+id: String) -> String:
  bosses.Art.name_of.k20(String.eq(id, "hellforge-slag-sovereign"), id)

def bosses.Art.name_of.k20(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "slag_sovereign"
    case False{}:
      bosses.Art.name_of.n21(id)

def bosses.Art.name_of.n21(+id: String) -> String:
  bosses.Art.name_of.k21(String.eq(id, "salvage-claimant"), id)

def bosses.Art.name_of.k21(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "salvage_claimant"
    case False{}:
      bosses.Art.name_of.n22(id)

def bosses.Art.name_of.n22(+id: String) -> String:
  bosses.Art.name_of.k22(String.eq(id, "shoggoth-heart"), id)

def bosses.Art.name_of.k22(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "shoggoth_heart"
    case False{}:
      bosses.Art.name_of.n23(id)

def bosses.Art.name_of.n23(+id: String) -> String:
  bosses.Art.name_of.k23(String.eq(id, "the-big-ripper"), id)

def bosses.Art.name_of.k23(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "big_ripper"
    case False{}:
      bosses.Art.name_of.n24(id)

def bosses.Art.name_of.n24(+id: String) -> String:
  bosses.Art.name_of.k24(String.eq(id, "unknown-memory-warden"), id)

def bosses.Art.name_of.k24(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "memory_warden"
    case False{}:
      bosses.Art.name_of.n25(id)

def bosses.Art.name_of.n25(+id: String) -> String:
  bosses.Art.name_of.k25(String.eq(id, "unknown-silence-warden"), id)

def bosses.Art.name_of.k25(hit: Bool, +id: String) -> String:
  match hit:
    case True{}:
      "silence_warden"
    case False{}:
      ""
