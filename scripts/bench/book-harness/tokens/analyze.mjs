// node analyze.mjs <label.jsonl>...  — per-run wire tokens under four tokenizers.
// Input per request = system + messages + tool schemas (JSON). "Fresh" input
// assumes perfect prefix caching: only the tokens past the previous request's
// messages are new (tool schemas and the system prompt are cached after hop 1).
import fs from "node:fs";
import path from "node:path";
import { fromPreTrained as ds } from "@lenml/tokenizer-deepseek_v4";
import { fromPreTrained as g4 } from "@lenml/tokenizer-gpt4o";
import { fromPreTrained as q3 } from "@lenml/tokenizer-qwen3";
import { fromPreTrained as l3 } from "@lenml/tokenizer-llama3_1";

export const TOKENIZERS = { deepseek_v4: ds(), gpt4o: g4(), qwen3: q3(), llama3: l3() };
const cache = new Map();
export function count(name, text) {
  if (!text) return 0;
  const key = name + "\u0000" + text;
  let n = cache.get(key);
  if (n === undefined) {
    n = TOKENIZERS[name].encode(text, { add_special_tokens: false }).length;
    cache.set(key, n);
  }
  return n;
}

function contentText(message) {
  const c = message.content;
  let text = Array.isArray(c) ? c.map((p) => p.text ?? "").join("") : c ?? "";
  for (const call of message.tool_calls ?? []) {
    text += `\n${call.function?.name ?? ""} ${call.function?.arguments ?? ""}`;
  }
  return text;
}

export function analyzeRun(file) {
  const rows = fs.readFileSync(file, "utf8").trim().split("\n").map((l) => JSON.parse(l));
  const out = {};
  for (const name of Object.keys(TOKENIZERS)) {
    let total = 0, fresh = 0, prevMessages = 0;
    let first = null;
    for (const [i, row] of rows.entries()) {
      const msgs = row.body.messages.map((m) => count(name, contentText(m)));
      const tools = count(name, JSON.stringify(row.body.tools ?? []));
      const system = row.body.messages
        .filter((m) => m.role === "system")
        .reduce((a, m) => a + count(name, contentText(m)), 0);
      const input = tools + msgs.reduce((a, b) => a + b, 0);
      total += input;
      fresh += i === 0 ? input : msgs.slice(prevMessages).reduce((a, b) => a + b, 0);
      prevMessages = msgs.length;
      if (i === 0) first = { input, tools, system, messages: input - tools };
    }
    out[name] = { requests: rows.length, first, total, fresh };
  }
  const reads = rows.filter((r) => JSON.stringify(r.action ?? "").includes("ledger://")).length;
  return { label: path.basename(file, ".jsonl"), requests: rows.length, ledgerReads: reads, bytesFirst: rows[0].bytes, bytesTotal: rows.reduce((a, r) => a + r.bytes, 0), tok: out };
}

if (process.argv[1].endsWith("analyze.mjs")) {
  const results = process.argv.slice(2).map(analyzeRun);
  console.log(JSON.stringify(results, null, 1));
}
