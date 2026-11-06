//! stop-slop：中文 AI 写作痕迹检测 CLI（stop-slop-zh 配套，只检测不改写）。

mod candidates;
mod report;
mod rules;
mod text;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use text::Doc;

const USAGE: &str = "stop-slop —— 中文 AI 写作痕迹检测（只检测，不改写）

用法：
  stop-slop detect <文件或目录> [--json] [--max-hits N]
  stop-slop detect --shots <shots.json|-> [--json] [--max-hits N]
  stop-slop candidates <文件> [--out 路径]
  stop-slop candidates --shots <shots.json|-> [--out 路径]
  stop-slop compare <改前> <改后> [--json]
  stop-slop --version

detect      分级报告：A 机械改 / B 建议改 / F 标点指纹 / C 只报告 / X 慎改 / H 人审
candidates  抽语义层候选 JSON（rename / cohesion / colon / iso3），交给模型按判据填标签
compare     改前改后各检测一遍，输出 Markdown 对比表

--shots     口播模式：输入 JSON 数组 [{id, text}, ...]（'-' 读 stdin），
            每个镜头当一段正文检测，命中带 shot 字段；默认列出全部命中

目录会递归读取 .md / .txt。规则说明见 https://github.com/leeguooooo/stop-slop-zh";

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if path.is_file() {
        out.push(path.to_path_buf());
        return Ok(());
    }
    let mut entries: Vec<_> = std::fs::read_dir(path)?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.path());
    for e in entries {
        let p = e.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" || name == "node_modules" {
            continue;
        }
        if p.is_dir() {
            collect(&p, out)?;
        } else if matches!(
            p.extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref(),
            Some("md" | "txt" | "markdown")
        ) {
            out.push(p);
        }
    }
    Ok(())
}

fn load(target: &str) -> Result<Vec<Doc>, String> {
    let root = Path::new(target);
    if !root.exists() {
        return Err(format!("路径不存在：{target}"));
    }
    let mut files = Vec::new();
    collect(root, &mut files).map_err(|e| format!("读取失败：{e}"))?;
    if files.is_empty() {
        return Err(format!("{target} 下没有 .md / .txt 文件"));
    }
    let mut docs = Vec::new();
    for f in files {
        let bytes = std::fs::read(&f).map_err(|e| format!("读取 {} 失败：{e}", f.display()))?;
        let src = String::from_utf8_lossy(&bytes);
        let name = if root.is_dir() {
            f.strip_prefix(root).unwrap_or(&f).display().to_string()
        } else {
            f.file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        docs.push(Doc::parse(&name, &src));
    }
    Ok(docs)
}

struct Opts {
    pos: Vec<String>,
    json: bool,
    max_hits: Option<usize>,
    out: Option<String>,
    shots: Option<String>,
}

/// 口播模式输入：镜头旁白列表。
#[derive(serde::Deserialize)]
struct Shot {
    id: String,
    text: String,
}

/// 把镜头拼成一篇 Markdown：每个镜头一段，返回文档和「段首行号 → 镜头 id」表。
fn shots_doc(src: &str) -> Result<(Doc, Vec<(usize, String)>), String> {
    let shots: Vec<Shot> =
        serde_json::from_str(src).map_err(|e| format!("--shots 不是 [{{id, text}}] 数组：{e}"))?;
    let mut md = String::new();
    let mut starts = Vec::new();
    let mut line = 1usize;
    for s in shots {
        // 旁白里的换行并成一段；去掉会被当成标题 / 列表 / 引用 / 表格的行首符号
        let t = s.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let t = t.trim_start_matches(['#', '>', '|', '-', '*', '+', ' ']);
        if t.is_empty() {
            continue;
        }
        starts.push((line, s.id));
        md.push_str(t);
        md.push_str("\n\n");
        line += 2;
    }
    if starts.is_empty() {
        return Err("--shots 里没有非空旁白".into());
    }
    Ok((Doc::parse("shots", &md), starts))
}

fn read_input(src: &str) -> Result<String, String> {
    if src == "-" {
        let mut b = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut b)
            .map_err(|e| format!("读取 stdin 失败：{e}"))?;
        Ok(b)
    } else {
        std::fs::read_to_string(src).map_err(|e| format!("读取 {src} 失败：{e}"))
    }
}

fn shot_of(starts: &[(usize, String)], line: usize) -> Option<String> {
    starts
        .iter()
        .rev()
        .find(|(l, _)| *l <= line)
        .map(|(_, id)| id.clone())
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        pos: Vec::new(),
        json: false,
        max_hits: None,
        out: None,
        shots: None,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--json" => o.json = true,
            "--max-hits" => {
                let v = it.next().ok_or("--max-hits 需要一个数字")?;
                o.max_hits = Some(v.parse().map_err(|_| format!("--max-hits 不是数字：{v}"))?);
            }
            "--shots" => o.shots = Some(it.next().ok_or("--shots 需要 JSON 文件路径或 -")?.clone()),
            "--out" | "-o" => o.out = Some(it.next().ok_or("--out 需要路径")?.clone()),
            s if s.starts_with("--") => return Err(format!("未知参数：{s}")),
            _ => o.pos.push(a.clone()),
        }
    }
    Ok(o)
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    match cmd.as_str() {
        "-V" | "--version" | "version" => {
            println!("stop-slop {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "-h" | "--help" | "help" => {
            println!("{USAGE}");
            Ok(())
        }
        "detect" => {
            let o = parse(&args[1..])?;
            let r = if let Some(src) = &o.shots {
                if !o.pos.is_empty() {
                    return Err("--shots 和文件参数只能二选一".into());
                }
                let (doc, starts) = shots_doc(&read_input(src)?)?;
                let mut r = rules::detect(&[doc], o.max_hits.unwrap_or(usize::MAX));
                r.meta.mode = "spoken".into();
                for item in &mut r.items {
                    for h in &mut item.hits {
                        h.shot = shot_of(&starts, h.line);
                    }
                }
                r
            } else {
                let [target] = o.pos.as_slice() else {
                    return Err(
                        "用法：stop-slop detect <文件或目录> [--json] [--max-hits N]".into(),
                    );
                };
                rules::detect(&load(target)?, o.max_hits.unwrap_or(5))
            };
            if o.json {
                println!("{}", serde_json::to_string_pretty(&r).unwrap());
            } else {
                print!("{}", report::text(&r));
            }
            Ok(())
        }
        "candidates" => {
            let o = parse(&args[1..])?;
            let c = if let Some(src) = &o.shots {
                if !o.pos.is_empty() {
                    return Err("--shots 和文件参数只能二选一".into());
                }
                let (doc, starts) = shots_doc(&read_input(src)?)?;
                let mut c = candidates::extract(&doc);
                c.meta["mode"] = "spoken".into();
                for v in c
                    .rename
                    .iter_mut()
                    .chain(c.cohesion.iter_mut())
                    .chain(c.colon.iter_mut())
                    .chain(c.iso3.iter_mut())
                {
                    if let Some(line) = v["line"].as_u64() {
                        let line = line as usize;
                        v["shot"] = shot_of(&starts, line).into();
                        if v["type"] == "cohesion" {
                            // 接口在两个镜头之间：上一个镜头是接口行之前最近的那个
                            v["prev_shot"] = shot_of(&starts, line.saturating_sub(1)).into();
                        }
                    }
                }
                c
            } else {
                let [target] = o.pos.as_slice() else {
                    return Err("用法：stop-slop candidates <文件> [--out 路径]".into());
                };
                if !Path::new(target).is_file() {
                    return Err("candidates 只接受单个文件".into());
                }
                let docs = load(target)?;
                candidates::extract(&docs[0])
            };
            let s = serde_json::to_string_pretty(&c).unwrap();
            match o.out {
                Some(p) => {
                    std::fs::write(&p, s).map_err(|e| format!("写入 {p} 失败：{e}"))?;
                    eprintln!("候选 {} 条 → {p}", c.meta["n_candidates"]);
                }
                None => println!("{s}"),
            }
            Ok(())
        }
        "compare" => {
            let o = parse(&args[1..])?;
            let [a, b] = o.pos.as_slice() else {
                return Err("用法：stop-slop compare <改前> <改后> [--json]".into());
            };
            let ra = rules::detect(&load(a)?, 0);
            let rb = rules::detect(&load(b)?, 0);
            if o.json {
                let v = serde_json::json!({"before": ra, "after": rb});
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
            } else {
                print!("{}", report::compare_md(&ra, &rb));
            }
            Ok(())
        }
        other => Err(format!("未知子命令：{other}\n\n{USAGE}")),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::rules::count_in;

    fn hit(src: &str, id: &str) {
        assert!(count_in(src, id) > 0, "{id} 应命中：{src}");
    }
    fn miss(src: &str, id: &str) {
        assert_eq!(count_in(src, id), 0, "{id} 不应命中：{src}");
    }

    #[test]
    fn r1_degree() {
        hit("这其实是一种非常重要的能力。", "R1");
        miss("这相当于一次重启。", "R1");
        miss("特别是在晚上。", "R1");
        miss("其实在压缩的过程中丢了信息。", "R1");
    }

    #[test]
    fn r2_binary() {
        hit("真正的壁垒不是技术，而是认知。", "R2");
        hit("看似简单，实则复杂。", "R2");
        miss("你是不是想太多了，而是要先睡一觉。", "R2");
        miss("这不是我的错。", "R2");
    }

    #[test]
    fn r4_slogan() {
        hit("这，就是坚持的力量。", "R4");
        hit("唯有坚持，方能成功。", "R4");
        miss("他坚持了三年。", "R4");
    }

    #[test]
    fn r6_meta() {
        hit("值得注意的是，接口变了。", "R6a");
        miss("接口在 v2 改了路径。", "R6a");
        hit("首先，打开设置。", "R6b");
        miss("他首先想到的是钱。", "R6b");
        hit("研究表明睡眠很重要。", "R6c");
        miss("张三 2024 年的论文测了 200 人。", "R6c");
        hit("家人们，今天聊聊。", "R6d");
        miss("我和家人吃饭。", "R6d");
        hit("要继续用旧模式我给你们开一个，说一声。", "R4a");
        miss("他一声不吭。", "R4a");
        hit("本文将介绍接入流程。", "R3a");
        hit("我们将于近期升级接口。", "R7a");
        hit("我们深入探讨了这个问题。", "R6e");
        miss("我们讨论了这个问题。", "R6e");
    }

    #[test]
    fn r10_rating() {
        hit("最反直觉的是这里。还有最重要的是那里。", "R10a");
        hit("有意思的是，它没用上。", "R10b");
        miss("它没有用上这个函数。", "R10b");
    }

    #[test]
    fn r11_prompt_colon() {
        hit("一句话总结：这个方案成本太高。", "R11");
        miss("## 总结：下一步", "R11");
        miss("原因有两个：", "R11");
        hit("问题在于：没人维护。", "R11");
        miss("```\n总结：代码里的不算\n```", "R11");
        // 行尾冒号归慎改
        hit("原因有两个：", "X3");
    }

    #[test]
    fn r12_translationese() {
        hit("当用户打开页面时，系统会加载数据。", "R12a");
        miss("用户打开页面的时候，系统会加载数据。", "R12a");
        miss("这是应当做的事，他当时很忙时，没空。", "R12a");
        hit("对于早期团队来说，招人最难。", "R12b");
        miss("早期团队最难的事是招人。", "R12b");
        hit("然而，事情没那么简单。", "R12c");
        miss("事情然而没那么简单。", "R12c");
    }

    #[test]
    fn b_level() {
        hit("我们对系统进行优化。", "R3");
        miss("我们优化了系统。", "R3");
        hit("写作是一种思考，是一种表达，更是一种修行。", "R2b");
        hit(
            "这个能够帮助团队在不增加人力的情况下显著提升审核速度的工具很好。",
            "R12d",
        );
        miss("这个问题是从零到一阶段最容易被忽视的问题。", "R12d");
        hit("我们支持苹果、香蕉、橘子三种水果。", "R13a");
        miss("- 苹果、香蕉、橘子", "R13a");
    }

    #[test]
    fn f_and_x() {
        hit("答案其实很简单——专注。", "F1");
        hit("# 一、背景", "X1");
        hit("这意味着成本会上升。", "X2");
        hit("你为什么这么做？", "X4");
        hit("它像一位不知疲倦的导师。", "H1");
    }

    #[test]
    fn quotes_tables_code_skipped() {
        miss("> 不是技术，而是认知。", "R2");
        miss("| 不是技术，而是认知 |", "R2");
        miss("`不是技术，而是认知`", "R2");
    }

    #[test]
    fn shots_map_hits_to_shot_ids() {
        let src = r##"[{"id":"hook","text":"你看这辆车。"},{"id":"empty","text":"  "},{"id":"rule","text":"# 这不是技术，\n而是认知。"}]"##;
        let (doc, starts) = crate::shots_doc(src).unwrap();
        let r = crate::rules::detect(&[doc], usize::MAX);
        let item = r.items.iter().find(|i| i.id == "R2").unwrap();
        assert_eq!(item.count, 1);
        assert_eq!(
            crate::shot_of(&starts, item.hits[0].line).as_deref(),
            Some("rule")
        );
        assert_eq!(starts.len(), 2);
        assert!(crate::shots_doc("{}").is_err());
    }

    #[test]
    fn shot_of_handles_boundaries() {
        let starts = vec![(1, "a".to_string()), (3, "b".to_string())];
        assert_eq!(crate::shot_of(&starts, 0), None);
        assert_eq!(crate::shot_of(&starts, 2).as_deref(), Some("a"));
        assert_eq!(crate::shot_of(&starts, 3).as_deref(), Some("b"));
    }
}
