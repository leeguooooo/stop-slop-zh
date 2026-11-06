//! Markdown / 纯文本的切分：行分类、段落分块、句子切分、汉字计数。
//!
//! 只有 prose（正文段）进入句段统计；标题单独计数；代码、引用、表格整块跳过。

use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    Blank,
    Code,
    Heading,
    List,
    Quote,
    Table,
    Prose,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub no: usize,
    /// 原始行文本
    pub raw: String,
    /// 去掉行内代码后的文本，用于规则匹配
    pub text: String,
    pub kind: LineKind,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub kind: LineKind,
    pub start: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Doc {
    pub name: String,
    pub lines: Vec<Line>,
    pub blocks: Vec<Block>,
}

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("内置正则有误"))
}

fn heading_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^\s{0,3}#{1,6}\s")
}

fn bold_heading_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^\s{0,3}\*\*[^*]{1,40}\*\*\s*$")
}

fn list_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"^\s*(?:[-*+]|\d+[.)、])\s")
}

fn inline_code_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    re(&R, r"`[^`]*`")
}

pub fn is_han(c: char) -> bool {
    matches!(c as u32,
        0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

pub fn han_count(s: &str) -> usize {
    s.chars().filter(|c| is_han(*c)).count()
}

fn classify(line: &str) -> LineKind {
    let t = line.trim_start();
    if t.trim().is_empty() {
        LineKind::Blank
    } else if heading_re().is_match(line) || bold_heading_re().is_match(line) {
        LineKind::Heading
    } else if t.starts_with('>') {
        LineKind::Quote
    } else if t.starts_with('|') {
        LineKind::Table
    } else if list_re().is_match(line) {
        LineKind::List
    } else {
        LineKind::Prose
    }
}

impl Doc {
    pub fn parse(name: &str, src: &str) -> Doc {
        let mut lines = Vec::new();
        let mut in_fence = false;
        // YAML frontmatter（首行 ---）整段当代码跳过
        let mut in_front = src.lines().next().map(str::trim_end) == Some("---");
        for (i, raw) in src.lines().enumerate() {
            let trimmed = raw.trim_start();
            let is_fence = trimmed.starts_with("```") || trimmed.starts_with("~~~");
            let kind = if in_front {
                if i > 0 && raw.trim_end() == "---" {
                    in_front = false;
                }
                LineKind::Code
            } else if is_fence {
                in_fence = !in_fence;
                LineKind::Code
            } else if in_fence {
                LineKind::Code
            } else {
                classify(raw)
            };
            let text = if kind == LineKind::Code {
                String::new()
            } else {
                inline_code_re().replace_all(raw, " ").into_owned()
            };
            lines.push(Line {
                no: i + 1,
                raw: raw.to_string(),
                text,
                kind,
            });
        }

        // 分块：空行切开；代码行自成一块；块类型取首行类型。
        let mut blocks: Vec<Block> = Vec::new();
        let mut cur: Option<Block> = None;
        for l in &lines {
            match l.kind {
                LineKind::Blank => {
                    if let Some(b) = cur.take() {
                        blocks.push(b);
                    }
                }
                _ => {
                    let need_new = match &cur {
                        None => true,
                        Some(b) => {
                            // 标题、代码与其他类型之间强制断块
                            (b.kind == LineKind::Code) != (l.kind == LineKind::Code)
                                || l.kind == LineKind::Heading
                                || b.kind == LineKind::Heading
                        }
                    };
                    if need_new {
                        if let Some(b) = cur.take() {
                            blocks.push(b);
                        }
                        cur = Some(Block {
                            kind: l.kind,
                            start: l.no,
                            text: l.text.clone(),
                        });
                    } else if let Some(b) = cur.as_mut() {
                        b.text.push('\n');
                        b.text.push_str(&l.text);
                    }
                }
            }
        }
        if let Some(b) = cur.take() {
            blocks.push(b);
        }

        Doc {
            name: name.to_string(),
            lines,
            blocks,
        }
    }

    /// 计入分母的汉字数：除代码以外的全部行
    pub fn han_total(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| l.kind != LineKind::Code)
            .map(|l| han_count(&l.text))
            .sum()
    }

    pub fn prose_blocks(&self) -> impl Iterator<Item = &Block> {
        self.blocks.iter().filter(|b| b.kind == LineKind::Prose)
    }
}

/// 按 。！？； 切句，保留 ≥4 字的片段（含问叹号本身不保留）
pub fn split_sentences(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if matches!(c, '。' | '！' | '？' | '；' | '\n') {
            let t = cur.trim().to_string();
            if t.chars().count() >= 4 {
                out.push(t);
            }
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    let t = cur.trim().to_string();
    if t.chars().count() >= 4 {
        out.push(t);
    }
    out
}

/// 去掉 markdown 强调 / 链接语法，留可读文本
pub fn strip_markdown(s: &str) -> String {
    static IMG: OnceLock<Regex> = OnceLock::new();
    static LINK: OnceLock<Regex> = OnceLock::new();
    static MARK: OnceLock<Regex> = OnceLock::new();
    let t = re(&IMG, r"!\[[^\]]*\]\([^)]*\)").replace_all(s, "");
    let t = re(&LINK, r"\[([^\]]*)\]\([^)]*\)").replace_all(&t, "$1");
    let t = re(&MARK, r"[*_#>]+").replace_all(&t, "");
    t.trim().to_string()
}

/// 截取前 n 个字符
pub fn clip(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_and_blocks() {
        let src = "# 标题\n\n正文第一段。\n续行。\n\n- 列表\n\n```\n代码 不是A而是B\n```\n\n> 引用\n| 表 |\n";
        let d = Doc::parse("t.md", src);
        let kinds: Vec<_> = d.blocks.iter().map(|b| b.kind).collect();
        assert_eq!(
            kinds,
            vec![
                LineKind::Heading,
                LineKind::Prose,
                LineKind::List,
                LineKind::Code,
                LineKind::Quote
            ]
        );
        assert_eq!(d.prose_blocks().count(), 1);
        // 代码块里的汉字不计入
        assert_eq!(d.han_total(), han_count("标题正文第一段续行列表引用表"));
    }

    #[test]
    fn frontmatter_skipped() {
        let d = Doc::parse("t.md", "---\nname: 不是A而是B\n---\n正文。\n");
        assert_eq!(d.prose_blocks().count(), 1);
        assert_eq!(d.han_total(), 2);
    }

    #[test]
    fn sentences() {
        let v = split_sentences("第一句话。第二句话！短。第三句话");
        assert_eq!(v, vec!["第一句话", "第二句话", "第三句话"]);
    }
}
