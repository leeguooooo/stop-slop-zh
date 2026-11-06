//! 检测规则与度量。
//!
//! 规则编号对应 stop-slop-zh 的 SKILL.md。人类 / AI 参考值出自
//! lieflat-less-ai-tone 公开的 629 篇对照 + 270 对同题同事实配对实测
//! （https://github.com/larashero3-dotcom/lieflat-less-ai-tone，RESEARCH.md），
//! 本程序是独立实现，正则与结构均为本项目自写。参考值是外部语料的中位 / 均值，
//! 只用来排序和提示，不能据此判断一篇文章是不是 AI 写的。

use crate::text::{clip, han_count, split_sentences, Doc, LineKind};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Level {
    A,
    B,
    C,
    F,
    X,
    H,
}

impl Level {
    pub fn title(self) -> &'static str {
        match self {
            Level::A => "机械改候选（固定字面，复核后可直接改）",
            Level::B => "建议改候选（要重组句子，逐条复核）",
            Level::C => "只报告（整篇分布，不逐句改）",
            Level::F => "标点指纹（分模型特征，不据此推断作者或模型）",
            Level::X => "慎改（外部实测人类用得一样多或更多，只计数，不要改）",
            Level::H => "人审（只列不评，默认保留）",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// 正文行 + 列表行
    Text,
    /// 正文行的行首
    ProseStart,
    /// 标题行
    Heading,
}

type Filter = fn(line: &str, start: usize, end: usize) -> bool;

struct LineRule {
    id: &'static str,
    level: Level,
    rule: &'static str,
    name: &'static str,
    pattern: &'static str,
    scope: Scope,
    filter: Option<Filter>,
    human: Option<&'static str>,
    ai: Option<&'static str>,
    advice: &'static str,
    evidence: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Hit {
    pub file: String,
    pub line: usize,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub level: Level,
    pub rule: String,
    pub name: String,
    pub count: usize,
    pub density: f64,
    /// "千字" 或 "百段"
    pub unit: String,
    pub human_ref: Option<String>,
    pub ai_ref: Option<String>,
    pub advice: String,
    pub evidence: Option<String>,
    /// 是否达到提示线
    pub flagged: bool,
    pub hits: Vec<Hit>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Structure {
    pub sentence_cv: Option<f64>,
    pub section_cv: Option<f64>,
    pub headings_per_k: f64,
    pub heading_pattern_ratio: Option<f64>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Meta {
    pub files: usize,
    pub han: usize,
    pub paragraphs: usize,
    pub headings: usize,
    pub sentences: usize,
    pub small_sample: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub version: String,
    pub meta: Meta,
    pub items: Vec<Item>,
    pub structure: Structure,
}

// ---------------- 过滤器（Rust regex 无环视，命中后用代码复核） ----------------

fn prev_char(line: &str, start: usize) -> Option<char> {
    line[..start].chars().next_back()
}

fn next_char(line: &str, end: usize) -> Option<char> {
    line[end..].chars().next()
}

/// 规则 1：排除「相当于」「特别是」「实在的」这类非程度用法
fn f_degree(line: &str, start: usize, end: usize) -> bool {
    let w = &line[start..end];
    let n = next_char(line, end);
    let p = prev_char(line, start);
    !(matches!(
        (w, n),
        ("相当", Some('于')) | ("特别", Some('是')) | ("实在", Some('的'))
    ) || (w == "实在" && p == Some('其')))
}

/// 规则 2：排除「是不是…」
fn f_not_shibushi(line: &str, start: usize, _end: usize) -> bool {
    prev_char(line, start) != Some('是')
}

/// 规则 11：冒号后还有内容才算提示语冒号；冒号在行尾的是「引列表」，归 X 级
fn f_colon_has_body(line: &str, _start: usize, end: usize) -> bool {
    !line[end..].trim().is_empty()
}

/// 规则 12「当…时，」：排除应当 / 相当 / 当然 / 当前 等词内的「当」
fn f_dang(line: &str, start: usize, _end: usize) -> bool {
    if let Some(p) = prev_char(line, start) {
        if "应相适恰正充担妥理承典便".contains(p) {
            return false;
        }
    }
    let after = &line[start + '当'.len_utf8()..];
    match after.chars().next() {
        Some(c) => !"然前时地下作成初中即场年天日事选做面代局红家真机务".contains(c),
        None => false,
    }
}

/// 对（X）而言：排除「对」作形容词 / 动词的常见误报（对的、对不起、对比）
fn f_topic_shell(line: &str, start: usize, _end: usize) -> bool {
    let after = &line[start..];
    !(after.starts_with("对比") || after.starts_with("对不") || after.starts_with("对的"))
}

/// 过长前置定语：中间段含「是」的多半是判断句（这个问题是……的问题），不是定语
fn f_long_attr(line: &str, start: usize, end: usize) -> bool {
    let m = &line[start..end];
    let body: String = m.chars().skip(2).collect();
    !body.contains('是') && han_count(m) >= 15
}

// ---------------- 规则表 ----------------

fn line_rules() -> Vec<LineRule> {
    vec![
        LineRule {
            id: "R1",
            level: Level::A,
            rule: "1",
            name: "程度副词 / 虚词",
            pattern: r"非常|十分|极其|相当|特别|格外|真的|确实|的确|实在|一定程度上|在某种意义上|本质上|客观来讲|坦白说",
            scope: Scope::Text,
            filter: Some(f_degree),
            human: None,
            ai: None,
            advice: "删掉后意思不变就删；每千字超过 3 个才算问题",
            evidence: None,
        },
        LineRule {
            id: "R2",
            level: Level::A,
            rule: "2",
            name: "二元对立 / 翻案腔",
            pattern: r"(?:不是|并非|不在于|与其说)[^，。！？；\n]{0,20}[，,]?\s*(?:而是|而在于|不如说)|看似[^。！？；\n]{1,20}实则|你以为[^。！？；\n]{1,30}其实|表面上?[^。！？；\n]{1,20}实际上",
            scope: Scope::Text,
            filter: Some(f_not_shibushi),
            human: Some("0.22"),
            ai: Some("0.73"),
            advice: "直接说 Y 是什么，删掉否定 X 的铺垫；一篇超过 2 处必拆",
            evidence: Some("两轮实测同向，配对复测 δ+0.88（GPT）/ +0.72（Opus），最强信号之一"),
        },
        LineRule {
            id: "R4",
            level: Level::A,
            rule: "4",
            name: "金句收尾 / 抒情升华",
            pattern: r"这，就是|这就是[^，。！？；\n]{1,12}的力量|唯有[^。！？；\n]{1,20}方能|让我们一起|弥足珍贵|在这个[^，。！？；\n]{1,12}的时代",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "删掉升华句，段落讲完事就结束",
            evidence: None,
        },
        LineRule {
            id: "R6a",
            level: Level::A,
            rule: "6",
            name: "元评论 / 强调虚壳 / AI 口头禅",
            pattern: r"让我们来|接下来我将|在本文中|下面我们来看|值得注意的是|值得一提的是|不得不说|毫无疑问|不容忽视|我们必须认识到|我必须诚实地说|这个问题是真实存在的",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "删掉预告和虚壳，直接讲事",
            evidence: None,
        },
        LineRule {
            id: "R6b",
            level: Level::A,
            rule: "6",
            name: "八股连接词",
            pattern: r"首先[，,]|其次[，,]|再者[，,]|综上所述|总而言之|由此可见|总的来说|概括而言",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "删或换；分点时每点开篇句写得不一样",
            evidence: Some("外部配对复测未测出人机差异，按可读性处理，不当作 AI 证据"),
        },
        LineRule {
            id: "R6c",
            level: Level::A,
            rule: "6",
            name: "无名归因",
            pattern: r"专家(?:认为|指出|表示)|研究(?:表明|显示)|数据显示|业内普遍认为|有人说|众所周知",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "补实名来源（人 / 机构 / 文献），补不出就删",
            evidence: None,
        },
        LineRule {
            id: "R6d",
            level: Level::A,
            rule: "6",
            name: "假亲切",
            pattern: r"朋友们|家人们|宝子们|亲爱的读者",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "删",
            evidence: None,
        },
        LineRule {
            id: "R6e",
            level: Level::A,
            rule: "6",
            name: "英译洋腔词",
            pattern: r"深入研究|深入探讨|见证了|格局|错综复杂|充满活力|无缝|至关重要",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "换回普通中文（delve / testament / landscape 的直译）",
            evidence: None,
        },
        LineRule {
            id: "R3a",
            level: Level::A,
            rule: "第三节",
            name: "文档自我描述",
            pattern: r"本文将|这篇文档是|看不懂就回来查|两点要知道|^读者[：:]",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "结论文档只放结论，删掉描述文档自己的话（第三节）",
            evidence: None,
        },
        LineRule {
            id: "R4a",
            level: Level::A,
            rule: "第四节",
            name: "回复类客套",
            pattern: r"说一声|随时找我|有需要(?:随时)?联系我|希望对你有(?:所)?帮助|有问题欢迎继续提|感谢理解",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "删掉邀约和客套，直接写这件事需要什么（第四节）",
            evidence: None,
        },
        LineRule {
            id: "R7a",
            level: Level::B,
            rule: "7",
            name: "时间状态含糊词",
            pattern: r"近期|逐步|有望|不久的将来",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "写清哪天、哪个版本、什么条件下",
            evidence: None,
        },
        LineRule {
            id: "R10a",
            level: Level::A,
            rule: "10",
            name: "评级悬念「最 XX 的是」",
            pattern: r"最\p{Han}{1,6}?的是",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "全文最多留 1 个给真正最重的点，其余砍前缀、让事实开句",
            evidence: None,
        },
        LineRule {
            id: "R10b",
            level: Level::A,
            rule: "10",
            name: "评级悬念词",
            pattern: r"有意思的是|更微妙的是|反直觉|出乎意料|令人惊讶|耐人寻味的是",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "删评级词，惊讶交给事实本身",
            evidence: None,
        },
        LineRule {
            id: "R11",
            level: Level::A,
            rule: "11",
            name: "提示语冒号",
            pattern: r"(?:一句话(?:总结|说|概括)|总结|小结|核心(?:是|在于)?|关键(?:是|在于)?|重点是|问题(?:是|在于)|原因(?:如下|有|在于)|答案是|本质(?:是|上)|我的(?:观点|判断|结论)|换句话说|也就是说)[：:]",
            scope: Scope::Text,
            filter: Some(f_colon_has_body),
            human: Some("0.08"),
            ai: Some("0.29"),
            advice: "提示语不带信息就删掉直接写；前半句承接上文时只把冒号换成句号或逗号",
            evidence: Some("两轮同向，语义层「结论提示冒号」AI 约 2.1 倍"),
        },
        LineRule {
            id: "R12a",
            level: Level::A,
            rule: "12",
            name: "翻译腔「当……时，」",
            pattern: r"当[^，。！？；\n]{2,20}时，",
            scope: Scope::Text,
            filter: Some(f_dang),
            human: None,
            ai: Some("0.26"),
            advice: "删「当」和「时」；确实要标时点的不改",
            evidence: Some("第一轮 18 种翻译腔里筛出的 4 种之一"),
        },
        LineRule {
            id: "R12b",
            level: Level::A,
            rule: "12",
            name: "翻译腔：前置话题壳",
            pattern: r"对于[^，。！？；\n]{1,15}来说|对[^，。！？；\n]{1,12}而言|就[^，。！？；\n]{1,12}而言",
            scope: Scope::Text,
            filter: Some(f_topic_shell),
            human: None,
            ai: Some("0.22"),
            advice: "后句本来就以它为主语时删壳子，对象直接作主语",
            evidence: Some("第一轮 18 种翻译腔里筛出的 4 种之一"),
        },
        LineRule {
            id: "R12c",
            level: Level::A,
            rule: "12",
            name: "翻译腔：段首连接词当路标",
            pattern: r"^\s*(?:然而|因此|此外|与此同时|换言之|总而言之)[，,]",
            scope: Scope::ProseStart,
            filter: None,
            human: None,
            ai: Some("0.18"),
            advice: "移到主语后，或换「不过」「其实」；论证确实需要路标的不改",
            evidence: Some("第一轮 18 种翻译腔里筛出的 4 种之一"),
        },
        LineRule {
            id: "R3",
            level: Level::B,
            rule: "3",
            name: "名词化（动词 + 抽象名词）",
            pattern: r"(?:进行了?(?:优化|改进|分析|讨论|研究|处理|调整|评估|总结|整合|沟通|改善)|(?:做出|作出)了?(?:选择|决定|贡献|努力|调整|改变|回应)|实现了?(?:增长|突破|提升|转型|飞跃)|采取了?(?:措施|行动)|加以(?:改进|优化|分析|处理|利用|区分)|给予了?(?:支持|关注|帮助|指导|肯定)|提供(?:支持|保障|帮助|指导))",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "能用本动词就用本动词：进行优化→优化，做出选择→选",
            evidence: Some("外部实测人机差异不显著（人类反而略多），按可读性处理，不当作 AI 证据"),
        },
        LineRule {
            id: "R2b",
            level: Level::B,
            rule: "2",
            name: "三件套排比",
            pattern: r"是[^，。！？；\n]{1,20}，(?:也)?是[^，。！？；\n]{1,20}，(?:更|还|也)是|既[^，。！？；\n]{1,15}又[^，。！？；\n]{1,15}还",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "留一句最有力的；三句意思真不同就换不同句式说",
            evidence: None,
        },
        LineRule {
            id: "R12d",
            level: Level::B,
            rule: "12",
            name: "翻译腔：过长前置定语",
            pattern: r"(?:一个|一种|一套|这种|这个)[^，。、；：！？\s]{15,40}的\p{Han}{2,5}",
            scope: Scope::Text,
            filter: Some(f_long_attr),
            human: None,
            ai: Some("0.42"),
            advice: "拆成两个分句，把中心名词提前",
            evidence: Some("配对复测前置定语长 δ+0.48（GPT）/ +0.54（Opus）"),
        },
        LineRule {
            id: "X1",
            level: Level::X,
            rule: "慎改",
            name: "序数词小标题",
            pattern: r"^\s{0,3}#{1,6}\s*(?:[一二三四五六七八九十]+[、.．]|\d+[.、．]\s*\S)",
            scope: Scope::Heading,
            filter: None,
            human: None,
            ai: None,
            advice: "不要改",
            evidence: Some("配对复测反转：人类 8.6% vs AI 4.9%"),
        },
        LineRule {
            id: "X2",
            level: Level::X,
            rule: "慎改",
            name: "「这意味着 / 这表明」复述",
            pattern: r"这意味着|这表明|这说明",
            scope: Scope::Text,
            filter: None,
            human: Some("0.027"),
            ai: Some("0.016"),
            advice: "不要改",
            evidence: Some("配对复测人类更多，三项口径全反向"),
        },
        LineRule {
            id: "X3",
            level: Level::X,
            rule: "慎改",
            name: "冒号引列表（冒号在行尾）",
            pattern: r"[：:]\s*$",
            scope: Scope::Text,
            filter: None,
            human: None,
            ai: None,
            advice: "不要改",
            evidence: Some("配对复测人类高一个量级，目录式长文的正常写法"),
        },
        LineRule {
            id: "H1",
            level: Level::H,
            rule: "人审",
            name: "拟人化喻体",
            pattern: r"(?:像|如同|好比|犹如|宛如)[^，。！？；\n]{0,6}一(?:位|名|个)[^，。！？；\n]{0,12}(?:导师|助手|管家|审查员|伙伴|灯塔|秘书|顾问|向导|守门人|教练|军师|舵手)",
            scope: Scope::Text,
            filter: None,
            human: Some("0.002"),
            ai: Some("0.018"),
            advice: "只列出，贴切与否由人判断；要改就平实写它实际做了什么",
            evidence: Some("两侧近饱和，差别在贴切度，模型判不准"),
        },
    ]
}

/// 同义词轮换组（启发式）
const SYN_GROUPS: &[&[&str]] = &[
    &["使用", "采用", "运用", "利用", "借助"],
    &["方法", "方式", "途径", "路径", "做法"],
    &["提升", "提高", "增强", "强化", "优化"],
    &["显示", "表明", "揭示", "反映"],
    &["问题", "难题", "挑战", "困境", "痛点"],
];

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn cv(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    if m == 0.0 {
        return None;
    }
    let var = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / n;
    Some(round3(var.sqrt() / m))
}

/// 句子结构指纹：逗号数、有无冒号、有无括号、长度档（每 15 字一档）
pub fn signature(s: &str) -> (usize, bool, bool, usize) {
    (
        s.matches('，').count(),
        s.contains('：') || s.contains(':'),
        s.contains('（') || s.contains('('),
        s.chars().count() / 15,
    )
}

/// 段内连续 n 句同构
pub fn isomorphic_runs(para: &str, n: usize) -> Vec<Vec<String>> {
    let sents: Vec<String> = split_sentences(para)
        .into_iter()
        .filter(|s| s.chars().count() > 10)
        .collect();
    let mut out = Vec::new();
    if sents.len() < n {
        return out;
    }
    for i in 0..=sents.len() - n {
        let s0 = signature(&sents[i]);
        if s0.0 >= 1 && sents[i + 1..i + n].iter().all(|s| signature(s) == s0) {
            out.push(sents[i..i + n].to_vec());
        }
    }
    out
}

struct Acc {
    hits: BTreeMap<&'static str, Vec<Hit>>,
}

impl Acc {
    fn push(&mut self, id: &'static str, file: &str, line: usize, excerpt: String) {
        self.hits.entry(id).or_default().push(Hit {
            file: file.to_string(),
            line,
            excerpt,
        });
    }
    fn take(&mut self, id: &str) -> Vec<Hit> {
        self.hits.remove(id).unwrap_or_default()
    }
}

fn context(line: &str, start: usize, end: usize) -> String {
    // 命中前后各带几个字，方便定位
    let before: String = {
        let v: Vec<char> = line[..start].chars().collect();
        v[v.len().saturating_sub(6)..].iter().collect()
    };
    let after: String = line[end..].chars().take(8).collect();
    clip(
        format!(
            "{}【{}】{}",
            before.trim_start(),
            &line[start..end],
            after.trim_end()
        )
        .trim(),
        70,
    )
}

pub fn detect(docs: &[Doc], max_hits: usize) -> Report {
    let rules = line_rules();
    let compiled: Vec<Regex> = rules
        .iter()
        .map(|r| Regex::new(r.pattern).expect("内置正则有误"))
        .collect();

    let mut acc = Acc {
        hits: BTreeMap::new(),
    };
    let mut han = 0usize;
    let mut paragraphs = 0usize;
    let mut headings = 0usize;
    let mut sent_lens: Vec<f64> = Vec::new();
    let mut sect_lens: Vec<f64> = Vec::new();
    let mut heading_patterned = 0usize;
    let pattern_heading = Regex::new(r"[：:？?]|为什么").unwrap();

    for d in docs {
        han += d.han_total();

        for l in &d.lines {
            let in_scope = |s: Scope| match s {
                Scope::Text => matches!(l.kind, LineKind::Prose | LineKind::List),
                Scope::ProseStart => l.kind == LineKind::Prose,
                Scope::Heading => l.kind == LineKind::Heading,
            };
            for (r, re) in rules.iter().zip(&compiled) {
                if !in_scope(r.scope) {
                    continue;
                }
                let target = if r.scope == Scope::Heading {
                    &l.raw
                } else {
                    &l.text
                };
                for m in re.find_iter(target) {
                    if let Some(f) = r.filter {
                        if !f(target, m.start(), m.end()) {
                            continue;
                        }
                    }
                    acc.push(r.id, &d.name, l.no, context(target, m.start(), m.end()));
                }
            }

            if matches!(l.kind, LineKind::Prose | LineKind::List) {
                // 标点指纹
                for (i, _) in l.text.match_indices("——") {
                    acc.push("F1", &d.name, l.no, context(&l.text, i, i + "——".len()));
                }
                for (i, _) in l.text.match_indices('；') {
                    acc.push(
                        "F2",
                        &d.name,
                        l.no,
                        context(&l.text, i, i + '；'.len_utf8()),
                    );
                }
                for (i, _) in l.text.match_indices("……") {
                    acc.push("F3", &d.name, l.no, context(&l.text, i, i + "……".len()));
                }
            }

            if l.kind == LineKind::Prose {
                // 顿号罗列：一个分句里 ≥2 个顿号
                for clause in l.text.split(['，', '。', '！', '？', '；', '：']) {
                    if clause.matches('、').count() >= 2 {
                        acc.push("R13a", &d.name, l.no, clip(clause.trim(), 60));
                    }
                }
                // 正文问句
                for s in l.text.split_inclusive(['。', '！', '？', '?']) {
                    if s.ends_with('？') || s.ends_with('?') {
                        acc.push("X4", &d.name, l.no, clip(s.trim(), 50));
                    }
                }
            }
        }

        // 段落级
        let mut sect = 0usize;
        for b in &d.blocks {
            match b.kind {
                LineKind::Heading => {
                    headings += 1;
                    let h = b.text.trim();
                    if pattern_heading.is_match(h) {
                        heading_patterned += 1;
                    }
                    if sect > 0 {
                        sect_lens.push(sect as f64);
                    }
                    sect = 0;
                }
                LineKind::Prose => {
                    paragraphs += 1;
                    sect += han_count(&b.text);
                    for s in split_sentences(&b.text) {
                        let n = han_count(&s);
                        if n > 0 {
                            sent_lens.push(n as f64);
                        }
                    }
                    for run in isomorphic_runs(&b.text, 2) {
                        acc.push("R13b", &d.name, b.start, clip(&run.join(" ／ "), 80));
                    }
                    for run in isomorphic_runs(&b.text, 3) {
                        acc.push("R13c", &d.name, b.start, clip(&run.join(" ／ "), 100));
                    }
                    for g in SYN_GROUPS {
                        let found: Vec<&str> =
                            g.iter().copied().filter(|w| b.text.contains(w)).collect();
                        if found.len() >= 2 {
                            acc.push("R14", &d.name, b.start, found.join(" / "));
                        }
                    }
                    let bangs = b.text.matches('！').count() + b.text.matches('!').count();
                    if bangs > 1 {
                        acc.push(
                            "F4",
                            &d.name,
                            b.start,
                            format!("本段 {} 个感叹号：{}", bangs, clip(b.text.trim(), 40)),
                        );
                    }
                }
                _ => {}
            }
        }
        if sect > 0 {
            sect_lens.push(sect as f64);
        }
    }

    let kk = (han as f64 / 1000.0).max(0.001);
    let hp = (paragraphs as f64 / 100.0).max(0.01);
    let mut items = Vec::new();

    let mut mk = |id: &str,
                  level: Level,
                  rule: &str,
                  name: &str,
                  hits: Vec<Hit>,
                  per100p: bool,
                  human: Option<&str>,
                  ai: Option<&str>,
                  advice: &str,
                  evidence: Option<&str>| {
        let count = hits.len();
        let density = round3(count as f64 / if per100p { hp } else { kk });
        let flagged = match (id, level) {
            (_, Level::X) | (_, Level::H) => false,
            ("R1", _) => density > 3.0,
            ("R10a", _) => count > 1,
            ("F1", _) => density > 2.0,
            ("F2", _) => density > 0.77,
            _ => count > 0,
        };
        let mut hits = hits;
        hits.truncate(max_hits);
        items.push(Item {
            id: id.to_string(),
            level,
            rule: rule.to_string(),
            name: name.to_string(),
            count,
            density,
            unit: if per100p { "百段" } else { "千字" }.to_string(),
            human_ref: human.map(str::to_string),
            ai_ref: ai.map(str::to_string),
            advice: advice.to_string(),
            evidence: evidence.map(str::to_string),
            flagged,
            hits,
        });
    };

    for r in &rules {
        let h = acc.take(r.id);
        mk(
            r.id, r.level, r.rule, r.name, h, false, r.human, r.ai, r.advice, r.evidence,
        );
    }

    mk(
        "R13a",
        Level::B,
        "13",
        "顿号罗列（一个分句 ≥3 项并列）",
        acc.take("R13a"),
        false,
        Some("1.78"),
        Some("3.21"),
        "能概括就概括；必须列全时改掉其中一项的句法；法规条目、配置清单不改",
        Some("两轮同向，配对复测密集并列 AI 1.45 倍"),
    );
    mk(
        "R13b",
        Level::B,
        "13",
        "相邻两句同构",
        acc.take("R13b"),
        true,
        Some("4.81"),
        Some("9.41"),
        "打散其中一句：合并、拆分、换语序；不删信息",
        Some("粗筛口径，误报多；精判跑 candidates"),
    );
    mk(
        "R13c",
        Level::B,
        "13",
        "连续三句同构",
        acc.take("R13c"),
        true,
        Some("0.41"),
        Some("0.84"),
        "拆掉其中一句的骨架；刻意修辞、清单、叙事推进不改",
        Some("高度同构三连 AI 29.4% vs 人类 10.7%；精判跑 candidates"),
    );
    mk(
        "R14",
        Level::B,
        "14",
        "同义词轮换（同段换说法）",
        acc.take("R14"),
        false,
        None,
        None,
        "同一个东西统一回原词；不同对象恰好用近义词的不改",
        Some("启发式，精判跑 candidates 的 rename 类"),
    );
    mk(
        "F1",
        Level::F,
        "9",
        "破折号「——」",
        acc.take("F1"),
        false,
        Some("0.83"),
        Some("2.02"),
        "超过 2/千字优先处理，否则逐处看；能换逗号句号就换",
        Some("分模型：GPT 偏少、Opus / DeepSeek 偏多，不据此推断模型"),
    );
    mk(
        "F2",
        Level::F,
        "9",
        "分号「；」",
        acc.take("F2"),
        false,
        Some("0.77"),
        Some("1.51"),
        "能安全拆成句号就拆；确有并列层级的保留",
        Some("GPT 偏多（δ+0.80），Opus 无差"),
    );
    mk(
        "F3",
        Level::F,
        "9",
        "省略号「……」",
        acc.take("F3"),
        false,
        None,
        None,
        "装欲言又止的删",
        None,
    );
    mk(
        "F4",
        Level::F,
        "9",
        "感叹号过密（一段 >1 个）",
        acc.take("F4"),
        false,
        None,
        None,
        "一段最多留一个",
        None,
    );
    mk(
        "X4",
        Level::X,
        "慎改",
        "正文问句",
        acc.take("X4"),
        false,
        Some("1.83"),
        Some("0.10"),
        "不要改",
        Some("人类是 AI 的十几倍，删了更不像人写"),
    );

    // 结构
    let sentence_cv = cv(&sent_lens);
    let section_cv = if sect_lens.len() >= 3 {
        cv(&sect_lens)
    } else {
        None
    };
    let headings_per_k = round3(headings as f64 / kk);
    let heading_pattern_ratio = if headings >= 3 {
        Some(round3(heading_patterned as f64 / headings as f64))
    } else {
        None
    };
    let mut notes = Vec::new();
    if let Some(c) = sentence_cv {
        if c < 0.46 {
            notes.push(format!(
                "句长 CV {c}（人类中位 0.54 / AI 0.46）偏低。只报告：不要为了参差机械改句长"
            ));
        }
    }
    if let Some(c) = section_cv {
        if c < 0.45 {
            notes.push(format!(
                "节长 CV {c}（人类中位 0.83 / AI 0.31）偏低，各节长度接近"
            ));
        }
    }
    if headings_per_k > 0.8 {
        notes.push(format!(
            "小标题 {headings_per_k}/千字（人类中位 0.32 / AI 1.26），切得偏碎"
        ));
    }
    if let Some(r) = heading_pattern_ratio {
        if r > 0.5 {
            notes.push(format!(
                "{:.0}% 的小标题含冒号 / 问号 / 为什么，标题句式偏整齐（规则 8 标题套路），把标题抄到一起看",
                r * 100.0
            ));
        }
    }

    Report {
        version: env!("CARGO_PKG_VERSION").to_string(),
        meta: Meta {
            files: docs.len(),
            han,
            paragraphs,
            headings,
            sentences: sent_lens.len(),
            small_sample: han < 300,
        },
        items,
        structure: Structure {
            sentence_cv,
            section_cv,
            headings_per_k,
            heading_pattern_ratio,
            notes,
        },
    }
}

/// 给测试用：单行文本命中某规则几次
#[cfg(test)]
pub fn count_in(src: &str, id: &str) -> usize {
    let d = Doc::parse("t.md", src);
    let r = detect(&[d], 1000);
    r.items
        .iter()
        .find(|i| i.id == id)
        .map(|i| i.count)
        .unwrap_or(0)
}
