"""在 HC3-Chinese 上跑 stop-slop detect，统计每条规则在人类回答和 ChatGPT 回答里的超线比例。

数据：https://huggingface.co/datasets/Hello-SimpleAI/HC3-Chinese 的 all.jsonl（约 21 MB，不随仓库发布）
用法：python3 -I tests/hc3/run.py all.jsonl cli/target/release/stop-slop [result.json]

只统计 ≥200 汉字的回答（太短的回答各项都到不了线）。"超线"= detect --json 里该项 flagged 为 true，
只看 A / B / F 三级。
"""

import collections
import json
import os
import re
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor

MIN_HAN = 200


def han(s):
    return len(re.findall(r"[一-鿿]", s))


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    data, binp = sys.argv[1], sys.argv[2]
    outp = sys.argv[3] if len(sys.argv) > 3 else None
    docs = []
    for i, line in enumerate(open(data, encoding="utf-8")):
        r = json.loads(line)
        for side in ("human", "chatgpt"):
            for j, a in enumerate(r.get(f"{side}_answers") or []):
                a = (a or "").replace("\r\n", "\n")
                if han(a) >= MIN_HAN:
                    docs.append((side, f"{i}-{j}", a))
    tmp = tempfile.mkdtemp()
    names = {}

    def run(d):
        side, k, text = d
        p = os.path.join(tmp, f"{side}-{k}.md")
        with open(p, "w", encoding="utf-8") as f:
            f.write(text)
        out = subprocess.run([binp, "detect", p, "--json"], capture_output=True, text=True).stdout
        os.remove(p)
        rep = json.loads(out)
        for it in rep["items"]:
            names.setdefault(it["id"], (it["level"], it["name"]))
        hit = [it["id"] for it in rep["items"] if it.get("flagged") and it["level"] in ("A", "B", "F")]
        return side, k, hit

    with ThreadPoolExecutor(os.cpu_count() or 4) as ex:
        res = list(ex.map(run, docs))
    if outp:
        json.dump(res, open(outp, "w"), ensure_ascii=False)
    n = {s: sum(1 for r in res if r[0] == s) for s in ("human", "chatgpt")}
    cnt = {s: collections.Counter(x for r in res if r[0] == s for x in r[2]) for s in n}
    anyf = {s: sum(1 for r in res if r[0] == s and r[2]) for s in n}
    print(f"人类 {n['human']} 篇，任一项超线 {anyf['human'] / n['human']:.1%}；"
          f"GPT-3.5 {n['chatgpt']} 篇，任一项超线 {anyf['chatgpt'] / n['chatgpt']:.1%}\n")
    print("| 项 | 级 | 名称 | 人类 | GPT-3.5 | 倍率 |")
    print("|---|---|---|---|---|---|")
    rows = []
    for k, (lv, nm) in names.items():
        if lv not in ("A", "B", "F"):
            continue
        h = cnt["human"][k] / n["human"]
        a = cnt["chatgpt"][k] / n["chatgpt"]
        rows.append((a / h if h else 0, k, lv, nm, h, a))
    for ratio, k, lv, nm, h, a in sorted(rows, reverse=True):
        print(f"| {k} | {lv} | {nm} | {h:.1%} | {a:.1%} | {ratio:.2f} |")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
