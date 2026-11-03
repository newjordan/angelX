import Base

# Competition-mode engagement for one operator turn, and the shell inventory
# classification that decides whether a command is a board digest or recon
# thrash. The host owns every text scan: it finds the current operator turn and
# the standing goal block, lowercases and searches them for the launch phrases,
# strips leading env assignments off a shell command, splits the argv into
# classified words, and reports whether a `git show` carries a listing flag or a
# `rev:path` blob word. This module owns the decisions that follow.
#
# Native twins, still the fallback and the parity oracle, in
# `cockpit/src/agent/harness/turn/competition.rs`:
#   * `competition_mode_trigger` — the env flag first, else the first launch
#     phrase in the current operator turn, else a competitive standing goal. The
#     leg answers with an index into the host's phrase table, so the phrase
#     string itself never crosses into Bend;
#   * `ls_cmd_is_recursive_listing` — an `ls` word followed by a recursive flag;
#   * `git_show_is_inventory_thrash` — only `git show REV:path` is a legal tip
#     blob digest;
#   * `shell_cmd_is_inventory_thrash` — the inventory command table plus the two
#     scans above.

# One whitespace-split argv word, classified by the host.
type Token is Data:
  # The word `ls` after any `/`-prefix, which opens the listing scan.
  Ls{}
  # Any other non-flag word (`cat`, `handoff.md`).
  Word{}
  # `--recursive`/`--recurse` (also `--recursive=`/`--recurse=`), or a short
  # cluster holding `r`.
  Recursive{}
  # Every other flag, and every leading env assignment.
  Plain{}

# What engaged competition mode. `Phrase` carries a table index, never text.
type Trigger is Data:
  Env{}
  Phrase{index: U32}
  Goal{}
  NoTrigger{}

# Where a shell command's first non-assignment word sits in the host's tables.
type First is Data:
  InventoryWord{}
  GitWord{}
  OtherWord{}

# The `git show` facts: both argv words present, a listing/summary flag, the
# `REV -- path` pathspec form, and a `REV:path` blob word.
type Git is Data:
  Git{has_git: Bool, has_show: Bool, list_flag: Bool, pathspec: Bool, blob: Bool}

type Verdict is Data:
  Hit{}
  Clear{}

def verdict(yes: Bool) -> Verdict:
  match yes:
    case True{}:
      Hit{}
    case False{}:
      Clear{}

# A recursive flag only counts after an `ls` word was seen, so the scan carries
# that state forward and stops at the first flag only a listing accepts.
def ls_scan(xs: List<Token>, +seen: Bool) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case h <> t:
      match h:
        case Ls{}:
          ls_scan(t, True{})
        case Word{}:
          ls_scan(t, seen)
        case Recursive{}:
          match seen:
            case True{}:
              True{}
            case False{}:
              ls_scan(t, seen)
        case Plain{}:
          ls_scan(t, seen)

def recursive_of(tokens: List<Token>) -> Verdict:
  verdict(ls_scan(tokens, False{}))

# `git show` is legal only as a tip blob read: a listing/summary flag or the
# `REV -- path` pathspec form is history inventory, and so is a show with no
# blob word at all.
def show_after_flags(pathspec: Bool, blob: Bool) -> Bool:
  match pathspec:
    case True{}:
      True{}
    case False{}:
      Bool.not(blob)

def show_flags(list_flag: Bool, pathspec: Bool, blob: Bool) -> Bool:
  match list_flag:
    case True{}:
      True{}
    case False{}:
      show_after_flags(pathspec, blob)

def show_words(has_show: Bool, list_flag: Bool, pathspec: Bool, blob: Bool) -> Bool:
  match has_show:
    case True{}:
      show_flags(list_flag, pathspec, blob)
    case False{}:
      False{}

def git_show_words(has_git: Bool, has_show: Bool, list_flag: Bool, pathspec: Bool, blob: Bool) -> Bool:
  match has_git:
    case True{}:
      show_words(has_show, list_flag, pathspec, blob)
    case False{}:
      False{}

def git_show(g: Git) -> Bool:
  match g:
    case Git{has_git, has_show, list_flag, pathspec, blob}:
      git_show_words(has_git, has_show, list_flag, pathspec, blob)

def any_true(xs: List<Bool>) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case h <> t:
      match h:
        case True{}:
          True{}
        case False{}:
          any_true(t)

# The first phrase the host matched wins, exactly as the native `find` does, and
# its list position is the answer.
def phrase_at(xs: List<Bool>, +at: U32) -> Trigger:
  match xs:
    case Nil{}:
      NoTrigger{}
    case h <> t:
      match h:
        case True{}:
          Phrase{at}
        case False{}:
          phrase_at(t, U32.add(at, 1))

def goal_flags(found: Bool) -> Trigger:
  match found:
    case True{}:
      Goal{}
    case False{}:
      NoTrigger{}

def goal_trigger(present: Bool, phrases: List<Bool>) -> Trigger:
  match present:
    case False{}:
      NoTrigger{}
    case True{}:
      goal_flags(any_true(phrases))

def after_phrases(found: Trigger, present: Bool, phrases: List<Bool>) -> Trigger:
  match found:
    case NoTrigger{}:
      goal_trigger(present, phrases)
    case Env{}:
      Env{}
    case Phrase{index}:
      Phrase{index}
    case Goal{}:
      Goal{}

def after_env(env: Bool, phrases: List<Bool>, present: Bool, goal_phrases: List<Bool>) -> Trigger:
  match env:
    case True{}:
      Env{}
    case False{}:
      after_phrases(phrase_at(phrases, 0), present, goal_phrases)

def trigger(env: Bool, phrases: List<Bool>, present: Bool, goal_phrases: List<Bool>) -> Trigger:
  after_env(env, phrases, present, goal_phrases)

# Every branch below answers `true`, so the order of the scans cannot change the
# answer. `GitWord` is only reached once the `git show` scan already said the
# command is not a blob read, which is why that branch is the host's argv table
# alone.
def shell_first(first: First, git_args: List<Bool>) -> Bool:
  match first:
    case InventoryWord{}:
      True{}
    case OtherWord{}:
      False{}
    case GitWord{}:
      any_true(git_args)

def shell_after_ls(show_hit: Bool, first: First, git_args: List<Bool>) -> Bool:
  match show_hit:
    case True{}:
      True{}
    case False{}:
      shell_first(first, git_args)

def shell_after_git(ls_hit: Bool, g: Git, first: First, git_args: List<Bool>) -> Bool:
  match ls_hit:
    case True{}:
      True{}
    case False{}:
      shell_after_ls(git_show(g), first, git_args)

def shell_thrash(thrash_hit: Bool, ls: List<Token>, g: Git, first: First, git_args: List<Bool>) -> Bool:
  match thrash_hit:
    case True{}:
      True{}
    case False{}:
      shell_after_git(ls_scan(ls, False{}), g, first, git_args)
