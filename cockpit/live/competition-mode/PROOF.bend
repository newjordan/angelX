import Base
import ./LAWS.bend as Laws

# Each law is a closed equality between literal decisions, so its proof only
# re-runs the same scans: no arithmetic beyond the U32 phrase index is assumed,
# and nothing is claimed beyond the eleven equalities above.

def Laws.an_empty_command_is_not_a_recursive_listing():
  {==}

def Laws.nothing_at_all_never_engages():
  {==}

def Laws.a_recursive_flag_needs_an_ls_word():
  {==}

def Laws.ls_holds_a_recursive_flag():
  {==}

def Laws.a_blob_digest_is_a_legal_show():
  {==}

def Laws.a_stat_show_is_inventory():
  {==}

def Laws.the_first_launch_phrase_names_its_index():
  {==}

def Laws.the_env_flag_outranks_every_phrase():
  {==}

def Laws.one_competitive_goal_word_engages():
  {==}

def Laws.an_inventory_word_is_thrash():
  {==}

def Laws.a_plain_command_is_not_thrash():
  {==}
