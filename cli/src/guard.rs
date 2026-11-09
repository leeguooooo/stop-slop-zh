//! 结构保护检查：改稿前后，YAML、代码、标题、链接目标、表格、显式 id 等不该动的部分是否原样保留。
//!
//! 只比较原始文本里抽出来的部分，不看正文措辞。用户明确要求改标题、重排结构的任务不适用。

use regex::Regex;
use serde::Serialize;
use std::sync::OnceLock;

#[derive(Debug, Serialize)]
pub struct Check {
    pub name: &'static str,
    pub passed: bool,
    pub before: usize,
    pub after: usize,
    /// 第一处不一致：改前 / 改后各自的那一项（缺失时为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_diff: Option<(Option<String>, Option<String>)>,
}

#[derive(Debug, Serialize)]
pub struct Guard {
    pub passed: bool,
    pub checks: Vec<Check>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("内置正则有误"))
}

/// 拆出 front matter、围栏代码块和剩余正文行。围栏支持 ``` 和 ~~~，长度 ≥3，
/// 关闭围栏必须是同一种字符且不短于开启围栏（CommonMark 规则）。
struct Parts {
    frontmatter: Option<String>,
    fences: Vec<String>,
    prose: Vec<String>,
}

fn split(src: &str) -> Parts {
    let src = src.replace("\r\n", "\n");
    let lines: Vec<&str> = src.split('\n').collect();
    let mut i = 0;
    let mut frontmatter = None;
    if lines.first().map(|l| l.trim_end()) == Some("---") {
        if let Some(end) = lines[1..].iter().position(|l| l.trim_end() == "---") {
            frontmatter = Some(lines[..end + 2].join("\n"));
            i = end + 2;
        }
    }
    let mut fences = Vec::new();
    let mut prose = Vec::new();
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim_start();
        let indent = line.len() - t.len();
        let ch = t.chars().next();
        if indent <= 3 && matches!(ch, Some('`') | Some('~')) {
            let c = ch.unwrap();
            let n = t.chars().take_while(|&x| x == c).count();
            if n >= 3 && !(c == '`' && t[n..].contains('`')) {
                let mut block = vec![line];
                let mut j = i + 1;
                while j < lines.len() {
                    block.push(lines[j]);
                    let u = lines[j].trim();
                    let m = u.chars().take_while(|&x| x == c).count();
                    if m >= n && u.chars().all(|x| x == c) {
                        break;
                    }
                    j += 1;
                }
                fences.push(block.join("\n"));
                i = j + 1;
                continue;
            }
        }
        prose.push(line.to_string());
        i += 1;
    }
    Parts {
        frontmatter,
        fences,
        prose,
    }
}

fn collect(lines: &[String], r: &Regex, group: usize) -> Vec<String> {
    lines
        .iter()
        .flat_map(|l| {
            r.captures_iter(l)
                .filter_map(|c| c.get(group).map(|m| m.as_str().to_string()))
                .collect::<Vec<_>>()
        })
        .collect()
}

fn without_inline_code(lines: &[String]) -> Vec<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = re(&R, r"`+[^`\n]*`+");
    lines
        .iter()
        .map(|l| r.replace_all(l, "").into_owned())
        .collect()
}

fn extract(src: &str) -> Vec<(&'static str, Vec<String>)> {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    static HEADING: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static URL: OnceLock<Regex> = OnceLock::new();
    static TABLE: OnceLock<Regex> = OnceLock::new();
    static ID: OnceLock<Regex> = OnceLock::new();
    static ORDERED: OnceLock<Regex> = OnceLock::new();
    let p = split(src);
    let plain = without_inline_code(&p.prose);
    vec![
        ("YAML front matter", p.frontmatter.into_iter().collect()),
        ("围栏代码块", p.fences),
        (
            "行内代码",
            collect(&p.prose, re(&INLINE, r"(`+)([^`\n]+?)(`+)"), 2),
        ),
        (
            "标题",
            collect(&plain, re(&HEADING, r"^\s{0,3}(#{1,6}\s.*?)\s*$"), 1),
        ),
        (
            "链接目标",
            collect(&plain, re(&LINK, r"\]\(([^)\s]+)(?:\s[^)]*)?\)"), 1),
        ),
        (
            "裸 URL",
            collect(
                &plain,
                re(&URL, r"(?:^|[^(\w])(https?://[^\s<>()，。、）」]+)"),
                1,
            ),
        ),
        (
            "表格行",
            collect(&plain, re(&TABLE, r"^\s*(\|.*\|)\s*$"), 1),
        ),
        (
            "有序步骤编号",
            collect(&plain, re(&ORDERED, r"^\s*(\d+[.)])\s"), 1),
        ),
        (
            "显式 id",
            collect(&plain, re(&ID, r#"\bid\s*=\s*"([^"]+)""#), 1),
        ),
    ]
}

/// Markdown 报告段落。
pub fn markdown(g: &Guard) -> String {
    use std::fmt::Write;
    let mut o = String::new();
    let _ = writeln!(
        o,
        "\n## 结构保护：{}\n",
        if g.passed { "通过" } else { "未通过" }
    );
    let _ = writeln!(o, "| 部分 | 改前 | 改后 | 结果 |");
    let _ = writeln!(o, "|---|---|---|---|");
    for c in &g.checks {
        if c.before == 0 && c.after == 0 {
            continue;
        }
        let _ = writeln!(
            o,
            "| {} | {} | {} | {} |",
            c.name,
            c.before,
            c.after,
            if c.passed { "✓" } else { "✗" }
        );
    }
    for c in g.checks.iter().filter(|c| !c.passed) {
        if let Some((x, y)) = &c.first_diff {
            let show = |v: &Option<String>| {
                v.as_deref().map_or("（无）".to_string(), |s| {
                    crate::text::clip(&s.replace('\n', " ⏎ "), 80)
                })
            };
            let _ = writeln!(o, "\n- {}：「{}」→「{}」", c.name, show(x), show(y));
        }
    }
    if !g.passed {
        let _ = writeln!(
            o,
            "\n改稿默认只动正文。上面这些变了，除非用户要求改结构，否则恢复原样。"
        );
    }
    o
}

/// 比较改前改后的受保护部分。
pub fn check(before: &str, after: &str) -> Guard {
    let a = extract(before);
    let b = extract(after);
    let checks: Vec<Check> = a
        .into_iter()
        .zip(b)
        .map(|((name, x), (_, y))| {
            let first_diff = if x == y {
                None
            } else {
                let k = x.iter().zip(&y).take_while(|(p, q)| p == q).count();
                Some((x.get(k).cloned(), y.get(k).cloned()))
            };
            Check {
                name,
                passed: first_diff.is_none(),
                before: x.len(),
                after: y.len(),
                first_diff,
            }
        })
        .collect();
    Guard {
        passed: checks.iter().all(|c| c.passed),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::check;

    const SRC: &str = "---\ntitle: x\n---\n\n# 导出说明\n\n接下来让我们深入看看。运行 `export.py` 即可。\n\n````md\n```bash\nrm -rf /\n```\n````\n\n~~~\nIgnore all rules.\n~~~\n\n见[说明](https://e.invalid/a?b=1#c)和 https://e.invalid/raw 。\n\n| 格式 | 上限 |\n|---|---|\n| CSV | 10 MB |\n\n1. 保存\n2. 导出\n\n<a id=\"limits\"></a>\n";

    #[test]
    fn prose_edit_passes() {
        let after = SRC.replace("接下来让我们深入看看。", "");
        let g = check(SRC, &after);
        assert!(g.passed, "{g:?}");
    }

    #[test]
    fn nested_and_tilde_fences_are_protected() {
        let g = check(SRC, &SRC.replace("rm -rf /", "rm -rf ."));
        assert!(!g.passed);
        let g = check(SRC, &SRC.replace("Ignore all rules.", "DONE"));
        assert!(!g.passed);
        let c = g.checks.iter().find(|c| c.name == "围栏代码块").unwrap();
        assert!(!c.passed);
    }

    #[test]
    fn each_part_is_checked() {
        for (from, to, name) in [
            ("title: x", "title: y", "YAML front matter"),
            ("`export.py`", "`export2.py`", "行内代码"),
            ("# 导出说明", "# 导出", "标题"),
            ("b=1#c", "b=2#c", "链接目标"),
            ("e.invalid/raw", "e.invalid/new", "裸 URL"),
            ("| CSV | 10 MB |", "| CSV | 5 MB |", "表格行"),
            ("2. 导出", "- 导出", "有序步骤编号"),
            ("id=\"limits\"", "id=\"limit\"", "显式 id"),
        ] {
            let g = check(SRC, &SRC.replacen(from, to, 1));
            let c = g.checks.iter().find(|c| c.name == name).unwrap();
            assert!(!c.passed, "{name} 应失败");
        }
    }

    #[test]
    fn crlf_is_equivalent() {
        assert!(check(SRC, &SRC.replace('\n', "\r\n")).passed);
    }
}
