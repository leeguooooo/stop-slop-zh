# 更新记录

中文 AI 文本的指纹在变。模型更新一轮，套话就换一批。这里记录黑名单和规则的变动。

新词收录标准：在三篇不同来源（不同模型 / 不同 prompt / 不同作者）的 AI 输出里都出现过，才进黑名单。避免误伤个人风格。

## 2026-06-02

### 新增

- **规则 9「管标点」**：补上标点指纹这一维，重点是破折号「——」滥用（英文 em-dash 的直接迁移）
- **`references/punctuation.md`**：标点指纹清单（破折号 / 省略号 / 感叹号 / 双引号刷概念 / 冒号八股），含自检方法
- 规则 1、2 内联了对应 references 的跳转链接
- frontmatter 补 `metadata`（trigger / author）块
- README 增加一行安装：`npx skills add leeguooooo/stop-slop-zh`（[skills CLI](https://www.skills.sh) 可发现根目录 SKILL.md）

### 修订

- 订正 SKILL.md 第六节的 references 数量：phrases 八大类 → 十二类、structures 十二种 → 十五类
- 订正 README 示例组数：7 组 → 6 组
- 统一 skill 名：`stop-ai-slop-zh` → `stop-slop-zh`（与仓库名一致）

### 致谢借鉴

- 标点指纹一节参考了 [pencil20388-eng/stop-slop-zh](https://github.com/pencil20388-eng/stop-slop-zh) 的 `banned-punctuation.md`，但去掉了"冒号一律禁用"等过激设定（中文冒号是合法标点）
- `metadata` 块、CHANGELOG 格式、规则内联链接参考自 [hardikpandya/stop-slop](https://github.com/hardikpandya/stop-slop)

## 初始版本

- 8 条核心改写规则 + 5 维评分量规
- 三层结构：词汇黑名单（phrases）/ 句式套路（structures）/ 改写示例（examples）
- 灵感来自 [hardikpandya/stop-slop](https://github.com/hardikpandya/stop-slop)，中文指纹完全重做
