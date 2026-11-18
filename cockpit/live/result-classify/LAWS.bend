import Base
import ./result-classify.bend as ResultClassify

law a_red_run_is_an_error:
  for error: Bool
  for verifier: Bool
  for has_exit: Bool
  for exit: U32
  for empty_marker: Bool
  for club_stream: Bool
  {Bool.or(error, Bool.not(ResultClassify.red_of(ResultClassify.decide(ResultClassify.Facts{error, verifier, has_exit, exit, empty_marker, club_stream})))) == True{} : Bool}

law a_clean_result_is_never_a_dispatch_failure:
  for verifier: Bool
  for has_exit: Bool
  for exit: U32
  for empty_marker: Bool
  for club_stream: Bool
  {ResultClassify.dispatch_of(ResultClassify.decide(ResultClassify.Facts{False{}, verifier, has_exit, exit, empty_marker, club_stream})) == False{} : Bool}

law an_unmarked_reply_is_never_an_empty_reply:
  for error: Bool
  for verifier: Bool
  for has_exit: Bool
  for exit: U32
  {ResultClassify.empty_of(ResultClassify.decide(ResultClassify.Facts{error, verifier, has_exit, exit, False{}, False{}})) == False{} : Bool}
