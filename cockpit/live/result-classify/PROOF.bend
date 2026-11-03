import Base
import ./LAWS.bend as Laws

def Laws.a_red_run_is_an_error(error, verifier, has_exit, exit, empty_marker, club_stream):
  match error:
    case True{}:
      {==}
    case False{}:
      {==}

def Laws.a_clean_result_is_never_a_dispatch_failure(verifier, has_exit, exit, empty_marker, club_stream):
  {==}

def Laws.an_unmarked_reply_is_never_an_empty_reply(error, verifier, has_exit, exit):
  {==}
