//! 文本报告与前后对比表。

use crate::rules::{Item, Level, Report};
use std::fmt::Write;

const LEVELS: [Level; 6] = [Level::A, Level::B, Level::F, Level::C, Level::X, Level::H];

fn refs(i: &Item) -> String {
    match (&i.human_ref, &i.ai_ref) {
        (Some(h), Some(a)) => format!("（人类≈{h}，AI≈{a}/{}）", i.unit),
        (Some(h), None) => format!("（人类≈{h}/{}）", i.unit),
        (None, Some(a)) => format!("（AI≈{a}/{}）", i.unit),
        _ => String::new(),
    }
}

pub fn text(r: &Report) -> String {
    let mut o = String::new();
    let m = &r.meta;
    let _ = writeln!(o, "stop-slop {} 检测报告（只检测，不改写）", r.version);
    let _ = writeln!(
        o,
        "样本：{} 个文件 / 汉字 {} / 正文段 {} / 句 {} / 小标题 {}",
        m.files, m.han, m.paragraphs, m.sentences, m.headings
    );
    if m.small_sample {
        let _ = writeln!(o, "⚠ 汉字少于 300，密度数字波动大，只看具体命中");
    }
    let _ = writeln!(
        o,
        "说明：命中是候选，逐条复核后再改。人类 / AI 参考值出自 lieflat-less-ai-tone 公开实测，只用来排序，不能据此判定作者是 AI。"
    );

    let mut quiet: Vec<String> = Vec::new();
    for lv in LEVELS {
        if lv == Level::C {
            let s = &r.structure;
            let _ = writeln!(o, "\n[C] {}", lv.title());
            let f = |x: Option<f64>| x.map_or("—".to_string(), |v| v.to_string());
            let _ = writeln!(
                o,
                "  句长 CV {}（人类中位 0.54 / AI 0.46）· 节长 CV {}（人类 0.83 / AI 0.31）· 小标题 {}/千字（人类 0.32 / AI 1.26）· 标题句式整齐度 {}",
                f(s.sentence_cv),
                f(s.section_cv),
                s.headings_per_k,
                f(s.heading_pattern_ratio)
            );
            for n in &s.notes {
                let _ = writeln!(o, "  ⚠ {n}");
            }
            continue;
        }
        let items: Vec<&Item> = r.items.iter().filter(|i| i.level == lv).collect();
        let shown: Vec<&&Item> = items.iter().filter(|i| i.count > 0).collect();
        for i in items.iter().filter(|i| i.count == 0) {
            quiet.push(i.name.clone());
        }
        if shown.is_empty() {
            continue;
        }
        let _ = writeln!(o, "\n[{:?}] {}", lv, lv.title());
        for i in shown {
            let rule = if i.rule.chars().all(|c| c.is_ascii_digit()) {
                format!("规则 {}", i.rule)
            } else {
                i.rule.clone()
            };
            let mark = if i.flagged {
                ""
            } else {
                "（未到提示线）"
            };
            let _ = writeln!(
                o,
                "  {} {}：{} 处，{}/{}{}{}",
                rule,
                i.name,
                i.count,
                i.density,
                i.unit,
                refs(i),
                if matches!(lv, Level::X | Level::H) {
                    ""
                } else {
                    mark
                }
            );
            let _ = writeln!(o, "    改法：{}", i.advice);
            if let Some(e) = &i.evidence {
                let _ = writeln!(o, "    依据：{e}");
            }
            for h in &i.hits {
                let _ = writeln!(o, "    · {}:{}  {}", h.file, h.line, h.excerpt);
            }
            if i.count > i.hits.len() {
                let _ = writeln!(
                    o,
                    "    …… 另 {} 处（--max-hits 调整）",
                    i.count - i.hits.len()
                );
            }
        }
    }
    if !quiet.is_empty() {
        let _ = writeln!(o, "\n无命中：{}", quiet.join("、"));
    }
    let _ = writeln!(
        o,
        "\n处理顺序：先 A 后 B，F 逐处看；C 只报告；X 不要改；H 交给人。"
    );
    o
}

fn arrow(a: f64, b: f64) -> &'static str {
    if (a - b).abs() < 1e-9 {
        "="
    } else if b < a {
        "↓"
    } else {
        "↑"
    }
}

pub fn compare_md(before: &Report, after: &Report) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "| 指标 | 改前 | 改后 | 人类参考 | 变化 |");
    let _ = writeln!(o, "|---|---|---|---|---|");
    for b in &before.items {
        let a = after.items.iter().find(|x| x.id == b.id);
        let Some(a) = a else { continue };
        if b.count == 0 && a.count == 0 {
            continue;
        }
        let href = b
            .human_ref
            .as_ref()
            .map_or("—".to_string(), |h| format!("{h}/{}", b.unit));
        let _ = writeln!(
            o,
            "| [{:?}] {} | {} 处（{}/{}） | {} 处（{}/{}） | {} | {} |",
            b.level,
            b.name,
            b.count,
            b.density,
            b.unit,
            a.count,
            a.density,
            a.unit,
            href,
            arrow(b.count as f64, a.count as f64)
        );
    }
    let sb = &before.structure;
    let sa = &after.structure;
    let rows: [(&str, Option<f64>, Option<f64>, &str); 4] = [
        ("[C] 句长 CV", sb.sentence_cv, sa.sentence_cv, "0.54"),
        ("[C] 节长 CV", sb.section_cv, sa.section_cv, "0.83"),
        (
            "[C] 小标题/千字",
            Some(sb.headings_per_k),
            Some(sa.headings_per_k),
            "0.32",
        ),
        (
            "[C] 标题句式整齐度",
            sb.heading_pattern_ratio,
            sa.heading_pattern_ratio,
            "—",
        ),
    ];
    for (n, x, y, h) in rows {
        let f = |v: Option<f64>| v.map_or("—".to_string(), |v| v.to_string());
        let ar = match (x, y) {
            (Some(x), Some(y)) => arrow(x, y),
            _ => "—",
        };
        let _ = writeln!(o, "| {n} | {} | {} | {h} | {ar} |", f(x), f(y));
    }
    let _ = writeln!(
        o,
        "\n改前汉字 {}，改后汉字 {}。A / B 级看是否清零或有逐处保留理由；结构类指标（句长 CV 等）只看有没有大幅摆动，不追求推到参考值，机械制造长短句比不改更糟。X 级数字变化说明误改了不该改的写法。",
        before.meta.han, after.meta.han
    );
    o
}
