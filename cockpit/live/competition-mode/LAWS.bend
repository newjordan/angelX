import Base
import ./competition-mode.bend as Competition

# These claims are about the decisions alone: the phrase flags, the classified
# argv words and the `git show` facts are literals here, so every law below is a
# closed equality rather than a claim about the host's text scans.

# Nothing to scan means no recursive listing and no engaged competition mode.
law an_empty_command_is_not_a_recursive_listing:
  {Competition.recursive_of(Nil{}) == Competition.Clear{} : Competition.Verdict}

law nothing_at_all_never_engages:
  {Competition.trigger(False{}, Nil{}, False{}, Nil{}) == Competition.NoTrigger{} : Competition.Trigger}

# A recursive flag only counts once the command already produced an `ls` word.
law a_recursive_flag_needs_an_ls_word:
  {Competition.recursive_of(Con{Competition.Recursive{}, Nil{}}) == Competition.Clear{} : Competition.Verdict}

law ls_holds_a_recursive_flag:
  {Competition.recursive_of(Con{Competition.Ls{}, Con{Competition.Recursive{}, Nil{}}}) == Competition.Hit{} : Competition.Verdict}

# `git show HEAD:board.md` is a legal tip blob read; a listing flag turns the
# same command into history inventory.
law a_blob_digest_is_a_legal_show:
  {Competition.verdict(Competition.git_show(Competition.Git{True{}, True{}, False{}, False{}, True{}})) == Competition.Clear{} : Competition.Verdict}

law a_stat_show_is_inventory:
  {Competition.verdict(Competition.git_show(Competition.Git{True{}, True{}, True{}, False{}, True{}})) == Competition.Hit{} : Competition.Verdict}

# The first matched launch phrase names its own table index, the env flag outranks
# every phrase, and one competitive goal word is enough when no phrase matched.
law the_first_launch_phrase_names_its_index:
  {Competition.trigger(False{}, Con{False{}, Con{True{}, Nil{}}}, False{}, Nil{}) == Competition.Phrase{1} : Competition.Trigger}

law the_env_flag_outranks_every_phrase:
  {Competition.trigger(True{}, Con{True{}, Nil{}}, True{}, Con{True{}, Nil{}}) == Competition.Env{} : Competition.Trigger}

law one_competitive_goal_word_engages:
  {Competition.trigger(False{}, Nil{}, True{}, Con{False{}, Con{True{}, Nil{}}}) == Competition.Goal{} : Competition.Trigger}

# An inventory command word is thrash with every other scan clean; a plain word
# with no listing, no `git show` and no inventory table entry is not.
law an_inventory_word_is_thrash:
  {Competition.shell_thrash(False{}, Nil{}, Competition.Git{False{}, False{}, False{}, False{}, False{}}, Competition.InventoryWord{}, Nil{}) == True{} : Bool}

law a_plain_command_is_not_thrash:
  {Competition.shell_thrash(False{}, Con{Competition.Word{}, Nil{}}, Competition.Git{False{}, False{}, False{}, False{}, False{}}, Competition.OtherWord{}, Nil{}) == False{} : Bool}
