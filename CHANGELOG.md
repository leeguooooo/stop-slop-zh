# 更新记录

中文 AI 文本的指纹在变。模型更新一轮，套话就换一批。这里记录黑名单和规则的变动。

新词收录标准：在三篇不同来源（不同模型 / 不同 prompt / 不同作者）的 AI 输出里都出现过，才进黑名单。避免误伤个人风格。

## 2026-06-25

### 新增

- **铁律「去具体 ≠ 编具体」**：SKILL.md 第二节开头加防伪栏，规则 7、交付前清单、examples.md 同步补约束。这是把规则交给 LLM 自动改写时最大的隐患——模型为"给具体"而幻觉假数字假场景，比套话更危险。手上没真实细节时只许三选一：退回平实陈述 / 留 `[作者补充]` 占位符 / 删句。
- **phrases.md 第 13 类「英译洋腔词」**：delve→深入研究、testament→见证、landscape→格局、underscore→强调、intricate→错综复杂、vibrant→充满活力、seamless→无缝等，含 13.1「英译 AI 口头禅」（我必须诚实地说 / 这个问题是真实存在的 等直译腔）。
- **phrases.md 第 14 类「无名归因（黄鼠狼话术）」**：专家认为 / 研究表明 / 数据显示 等无实名背书，要么补来源要么删。规则 6 与交付前清单同步收录。
- **困惑度 / 突发性框架**：SKILL.md 第一节补两句可量化定义，给「变」维（句长变化＝突发性）和「直 / 实」维一个理论支点。
- **文风 JSON 技法**：README 用法建议新增——喂 20–30 篇旧文提炼个人文风 JSON，长期复用，让模型照作者底色改而非 AI 默认底色。

### 修订

- references 计数：phrases 十二类 → 十四类（SKILL.md 第六节 + README 结构块同步）

### 致谢借鉴

- 英译洋腔词、无名归因、困惑度/突发性、文风 JSON 四项参考了一份《如何去除中文文章的 AI 味》深度研究综述（少数派 / 知乎 / 维基百科 AI 特征页 / 腾讯朱雀检测等来源）。该综述自陈验证环节因限流不充分，故只采纳了与本 Skill 互补、且可独立判断成立的条目。

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
