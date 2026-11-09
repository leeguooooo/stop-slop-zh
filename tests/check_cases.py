"""对照 tests/cases.json 检查改写结果里该留的留了没有、该删的删了没有。

用法：python3 tests/check_cases.py tests/cases.json outputs.json
outputs.json 是 {"<id>": "<终稿>", ...}。只做字符串级检查，`review` 一栏要人或独立审稿模型读。
"""

import json
import sys


def main():
    if len(sys.argv) != 3:
        print(__doc__)
        return 2
    cases = json.load(open(sys.argv[1], encoding="utf-8"))
    outputs = json.load(open(sys.argv[2], encoding="utf-8"))
    failed = 0
    for c in cases:
        out = outputs.get(c["id"])
        if out is None:
            print(f"[缺失] {c['id']}")
            failed += 1
            continue
        lost = [k for k in c["keep"] if k not in out]
        left = [k for k in c["forbid"] if k in out]
        ok = not lost and not left
        failed += not ok
        print(f"[{'通过' if ok else '失败'}] {c['id']}")
        if lost:
            print(f"    丢了：{'、'.join(lost)}")
        if left:
            print(f"    没删：{'、'.join(left)}")
        print(f"    人审：{c['review']}")
    print(f"\n{len(cases) - failed}/{len(cases)} 通过字符串检查")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
