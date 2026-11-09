# 测试

这里放两类检查，都不判断文章是不是 AI 写的。

## 改稿保真回归：`cases.json`

20 条短用例，查的是"去 AI 味的时候有没有把意思改坏"：计划写成完成、删掉限定语、删掉作者判断、❌ 被当装饰删掉、润色稿里留占位符、待改文本里的指令被执行等。大多来自 2026-11 的一次实测，当时的 SKILL.md 在这些地方出过错。

每条用例有四个字段：

- `request` / `input`：交给模型的用户指令和原文
- `keep`：终稿里必须还在的字符串
- `forbid`：终稿里不该再出现的字符串
- `review`：字符串检查不了、要人读的那一条

跑法：

1. 让 agent 加载仓库根目录的 `SKILL.md`，逐条执行 `request` + `input`。**不要把 `keep` / `forbid` / `review` 给它看**，最好让它只读到前两个字段
2. 把终稿存成 `{"<id>": "<终稿>", ...}`，只放正文，正文外的说明不放进去
3. 运行

   ```bash
   python3 tests/check_cases.py tests/cases.json outputs.json
   ```

4. 逐条读 `review`。可以交给一个没看过 SKILL.md 的模型审：告诉它"改写的人可能悄悄改了意思"，让它对照原文找新增、遗漏和强化

字符串检查通过不等于改得好。单次运行也不代表稳定通过率，换模型、换温度都要重跑。

## 结构保护：`stop-slop compare --strict`

改文件时，YAML front matter、围栏代码块（含 `~~~` 和更长的围栏）、行内代码、标题、链接目标、裸 URL、表格行、有序步骤编号和显式 id 应该原样保留：

```bash
stop-slop compare 改前.md 改后.md --strict   # 结构变了返回非零
```

用户明确要求改标题、重排结构的任务不适用。

## 误报测试：`hc3/`

检测器在 HC3-Chinese 真人回答上的触发率，见 [hc3/README.md](hc3/README.md)。

## 致谢

用例设计参考了 [op7418/humanizer-zh](https://github.com/op7418/humanizer-zh) 的 `tests/`（保留边界的用例和结构检查脚本），以及 [tangwenwen-md/chinese-de-ai-writing](https://github.com/tangwenwen-md/chinese-de-ai-writing) 记录的高热惊厥科普改稿事故。用例文本是本项目重写的。
