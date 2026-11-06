//! 语义层候选抽取：脚本只负责把候选摆出来，标签由会话里的模型按封闭判据填写。
//! 四类：rename（同义重命名，段级）、cohesion（段间接口）、colon（冒号句）、iso3（三连同构粗筛）。

use crate::rules::isomorphic_runs;
use crate::text::{clip, han_count, split_sentences, strip_markdown, Doc, LineKind};
use serde::Serialize;
use serde_json::{json, Value};

const CAP_RENAME: usize = 60;
const CAP_COHESION: usize = 60;
const CAP_COLON: usize = 80;
const CAP_ISO3: usize = 60;

#[derive(Serialize)]
pub struct Candidates {
    pub meta: Value,
    pub rename: Vec<Value>,
    pub cohesion: Vec<Value>,
    pub colon: Vec<Value>,
    pub iso3: Vec<Value>,
}

fn blank_labels(mut v: Value) -> Value {
    if let Value::Object(m) = &mut v {
        m.insert("label".into(), json!(""));
        m.insert("confidence".into(), json!(""));
        m.insert("reason".into(), json!(""));
    }
    v
}

/// 超过上限时均匀抽样，避免只看文章开头
fn sample(v: Vec<Value>, cap: usize) -> (Vec<Value>, bool) {
    if v.len() <= cap {
        return (v, false);
    }
    let step = v.len() as f64 / cap as f64;
    let out = (0..cap)
        .map(|i| v[(i as f64 * step) as usize].clone())
        .collect();
    (out, true)
}

/// 正文里的冒号：全角「：」，或两侧都是汉字的半角「:」（避开网址、时间）
fn colon_positions(s: &str) -> Vec<usize> {
    let chars: Vec<(usize, char)> = s.char_indices().collect();
    let mut out = Vec::new();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let ok = match c {
            '：' => true,
            ':' => {
                let p = k.checked_sub(1).map(|j| chars[j].1);
                let n = chars.get(k + 1).map(|x| x.1);
                matches!((p, n), (Some(a), Some(b)) if crate::text::is_han(a) && crate::text::is_han(b))
            }
            _ => false,
        };
        if ok {
            out.push(i);
        }
    }
    out
}

pub fn extract(doc: &Doc) -> Candidates {
    let paras: Vec<(usize, String)> = doc
        .prose_blocks()
        .map(|b| (b.start, strip_markdown(&b.text)))
        .filter(|(_, t)| han_count(t) >= 20)
        .collect();

    let mut rename = Vec::new();
    for (line, t) in &paras {
        if han_count(t) >= 30 {
            rename.push(blank_labels(
                json!({"type": "rename", "line": line, "para": t}),
            ));
        }
    }

    let mut cohesion = Vec::new();
    for w in paras.windows(2) {
        let prev = split_sentences(&w[0].1);
        let next = split_sentences(&w[1].1);
        let tail = prev.last().cloned().unwrap_or_else(|| clip(&w[0].1, 60));
        let head = next.first().cloned().unwrap_or_else(|| clip(&w[1].1, 60));
        cohesion.push(blank_labels(json!({
            "type": "cohesion", "line": w[1].0,
            "prev_tail": tail, "next_head": head
        })));
    }

    let mut colon = Vec::new();
    for l in &doc.lines {
        if l.kind != LineKind::Prose {
            continue;
        }
        let t = &l.text;
        for pos in colon_positions(t) {
            let before: String = {
                let seg = &t[..pos];
                let start = seg
                    .rfind(['，', '。', '！', '？', '；', '：'])
                    .map(|i| i + seg[i..].chars().next().map_or(0, char::len_utf8))
                    .unwrap_or(0);
                let v: Vec<char> = seg[start..].chars().collect();
                v[v.len().saturating_sub(30)..].iter().collect()
            };
            let after_start = pos + t[pos..].chars().next().map_or(1, char::len_utf8);
            let after: String = t[after_start..].chars().take(60).collect();
            if after.trim().is_empty() {
                continue; // 行尾冒号引列表，慎改项，不进候选
            }
            colon.push(blank_labels(json!({
                "type": "colon", "line": l.no,
                "colon_before": before.trim(), "colon_after": after.trim(),
                "sentence": clip(strip_markdown(t).as_str(), 120)
            })));
        }
    }

    let mut iso3 = Vec::new();
    for b in doc.prose_blocks() {
        for run in isomorphic_runs(&b.text, 3) {
            iso3.push(blank_labels(
                json!({"type": "iso3", "line": b.start, "sents": run}),
            ));
        }
    }

    let totals = json!({
        "rename": rename.len(), "cohesion": cohesion.len(),
        "colon": colon.len(), "iso3": iso3.len()
    });
    let (rename, t1) = sample(rename, CAP_RENAME);
    let (cohesion, t2) = sample(cohesion, CAP_COHESION);
    let (colon, t3) = sample(colon, CAP_COLON);
    let (iso3, t4) = sample(iso3, CAP_ISO3);
    let n = rename.len() + cohesion.len() + colon.len() + iso3.len();

    Candidates {
        meta: json!({
            "file": doc.name,
            "version": env!("CARGO_PKG_VERSION"),
            "n_candidates": n,
            "totals": totals,
            "truncated": {"rename": t1, "cohesion": t2, "colon": t3, "iso3": t4},
            "instructions": "按 references/semantic.md 的封闭标签逐条填写 label / confidence（高/中/低）/ reason；只用判据里列出的标签，拿不准选兜底标签"
        }),
        rename,
        cohesion,
        colon,
        iso3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_four_kinds() {
        let src = "这是第一段，内容足够长，用来测试同义重命名候选是否会被抽出来，我们采用这个方法。\n\n\
                   第二段接着说，这里有一个冒号：后面是展开的内容，足够长才行，再补几个字凑数凑数。\n\n\
                   代码地址 http://example.com 不算冒号。\n";
        let d = Doc::parse("t.md", src);
        let c = extract(&d);
        assert_eq!(c.rename.len(), 2);
        assert_eq!(c.cohesion.len(), 1);
        assert_eq!(c.colon.len(), 1);
        assert_eq!(c.rename[0]["label"], "");
    }

    #[test]
    fn sampling_caps() {
        let v: Vec<Value> = (0..200).map(|i| json!(i)).collect();
        let (s, t) = sample(v, 60);
        assert_eq!(s.len(), 60);
        assert!(t);
        assert_eq!(s[59], json!(196));
    }
}
