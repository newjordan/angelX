import Base

type Token is Data:
  Ls{}
  Word{}
  Recursive{}
  Plain{}

type Trigger is Data:
  Env{}
  Phrase{index: U32}
  Goal{}
  NoTrigger{}

type First is Data:
  InventoryWord{}
  GitWord{}
  OtherWord{}

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
