// node curve.mjs <tokenizer> <label.jsonl>... — cumulative total and fresh input at checkpoints.
import fs from "node:fs";
import path from "node:path";
import { count } from "./analyze.mjs";
const [name, ...files] = process.argv.slice(2);
const checkpoints = [7, 25, 50, 100, 150, 213];
const text = (m) => {
  const c = m.content;
  let t = Array.isArray(c) ? c.map((p) => p.text ?? "").join("") : c ?? "";
  for (const call of m.tool_calls ?? []) t += `\n${call.function?.name ?? ""} ${call.function?.arguments ?? ""}`;
  return t;
};
for (const file of files) {
  const rows = fs.readFileSync(file, "utf8").trim().split("\n").map((l) => JSON.parse(l));
  let total = 0, fresh = 0, prev = 0, tools = 0;
  const out = [];
  const perMsg = [];
  for (const [i, row] of rows.entries()) {
    const msgs = row.body.messages;
    while (perMsg.length < msgs.length) perMsg.push(count(name, text(msgs[perMsg.length])));
    // a message's text never changes once sent (asserted by the harness); reuse its count
    const t = count(name, JSON.stringify(row.body.tools ?? []));
    const input = t + perMsg.slice(0, msgs.length).reduce((a, b) => a + b, 0);
    total += input;
    fresh += i === 0 ? input : perMsg.slice(prev, msgs.length).reduce((a, b) => a + b, 0);
    prev = msgs.length;
    if (checkpoints.includes(i + 1)) out.push(`${i + 1}: total ${total} fresh ${fresh} ctx ${input}`);
  }
  console.log(path.basename(file, ".jsonl"), "|", out.join(" | "));
}
