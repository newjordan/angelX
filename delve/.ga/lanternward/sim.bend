import Base
import ../../lib/bj.bend as B
import ../../lib/j.bend as J
import ./rules.bend as R

def bots?(+ids: List<&2, U32>, +s: J.Val, +r: J.Val, +o: J.Val) -> J.Val:
  match ids:
    case Nil{}:
      o
    case Con{+id, +t}:
      bots(t, s, r, J.set(o, U32.show(id), R.lanternward.bot.Session.bot_input(s, id, r)))

def players?(+n: U32) -> J.Val:
  players.go(n, J.obj())

def players.go?(+n: U32, +o: J.Val) -> J.Val:
  match n:
    case 0:
      o
    case _:
      players.go((n - 1 : U32), J.set(o, U32.show(n), J.set(J.set(J.obj(), "away", J.JBool{False{}}), "stone", J.JBool{False{}})))

def report?(+s: J.Val, +tick: U32) -> String:
  "tick " ++ U32.show(tick) ++ " room " ++ U32.show(J.u32(J.get(s, "room"))) ++ " stage " ++ U32.show(J.u32(J.get(s, "stage")))
    ++ " seats " ++ U32.show(R.lanternward.seat_count(s)) ++ " foes " ++ U32.show(J.len(J.get(s, "foes")))
    ++ " done " ++ B.text(J.get(s, "done")) ++ " tries " ++ U32.show(J.u32(J.get(s, "tries")))

def loop?(+s: J.Val, +heroes: J.Val, +r: J.Val, +tick: U32, +limit: U32) -> IO(Unit):
  loop.go(U32.is_ge(tick, limit), s, heroes, r, tick, limit)

def loop.go?(done: Bool, +s: J.Val, +heroes: J.Val, +r: J.Val, +tick: U32, +limit: U32) -> IO(Unit):
  match done:
    case True{}:
      IO.print(report(s, tick))
    case False{}:
      loop.step(R.lanternward.Session.step(s, bots(R.ga.ids(J.get(s, "seats")), s, r, J.obj()), heroes, r), heroes, r, tick, limit)

def loop.step?(+s: J.Val, +heroes: J.Val, +r: J.Val, +tick: U32, +limit: U32) -> IO(Unit):
  loop.say(U32.is_eq(U32.mod(tick, 200), 0), R.lanternward.Session.out(s), heroes, r, tick, limit)

def loop.say?(show: Bool, +s: J.Val, +heroes: J.Val, +r: J.Val, +tick: U32, +limit: U32) -> IO(Unit):
  match show:
    case True{}:
      do IO<Unit>:
        IO.print(report(s, tick) ++ " problem " ++ B.text(R.lanternward.Session.problem(s)))
        loop(s, heroes, r, (tick + 1 : U32), limit)
    case False{}:
      loop(s, heroes, r, (tick + 1 : U32), limit)

def main?() -> IO(Unit):
  loop(R.lanternward.Session.new(players(2)), players(2), R.lanternward.Rules.default.full(), 0, 400)
