//! จัดการข้อความ SQL ของผู้ใช้ก่อนส่งไป Postgres (ฟังก์ชันล้วน ไม่แตะ DB):
//! - ตัวช่วยค้นด้วยค่าที่เข้ารหัส `:hn('66471')` / `:nap_no('...')` → `'\x<blind index>'::bytea`
//!   (คอลัมน์เข้ารหัสเทียบด้วย WHERE ตรง ๆ ไม่ได้ ต้องเทียบ `hn_idx` / `nap_no_idx` ด้วย blind index)
//! - คำแรกของคำสั่ง (ข้าม comment) — ใช้ตัดสินว่าเป็นคำสั่งอ่านข้อมูลไหม และครอบด้วย LIMIT ได้ไหม
//! - มีหลายคำสั่งคั่นด้วย `;` ไหม (หน้า SQL รันทีละคำสั่ง)
//!
//! การสแกนข้าม string literal (`'...'`, `E'...'`), dollar quote (`$tag$...$tag$`), identifier (`"..."`) และ comment
//! (`--`, `/* */` ซ้อนได้) จึงไม่แทนตัวช่วยที่อยู่ในข้อความหรือ comment
//!
//! ข้อความที่แทนลงไปเป็น hex ของ blind index ที่คำนวณเอง (มีแค่ 0-9a-f) จึงไม่เปิดช่อง injection — ส่วนตัว SQL
//! ทั้งหมดเป็นของผู้ใช้อยู่แล้วโดยเจตนา (หน้า SQL) แนวป้องกันอยู่ที่สิทธิ์ของ role (setup.rs)

use std::fmt::Write as _;

use crate::domain::patient_crypto::{AAD_HN, AAD_NAP_NO};

/// ตัวช่วยที่รองรับ: ชื่อหลัง `:` และ AAD ของคอลัมน์ที่ blind index ผูกไว้
const HELPERS: [(&str, &str); 2] = [("hn", AAD_HN), ("nap_no", AAD_NAP_NO)];

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum HelperError {
    /// เขียน `:hn(` แต่ไม่ตามด้วย `'ค่า')` — ไม่แนบค่าที่พิมพ์ลงใน error (อาจเป็น HN)
    #[error("invalid :{0}('...') helper")]
    Malformed(&'static str),
}

/// SQL หลังแทนตัวช่วยแล้ว
#[derive(Debug)]
pub struct Expanded {
    pub sql: String,
    /// จำนวนตัวช่วยที่แทน — มากกว่า 0 แปลว่าตำแหน่งตัวอักษรใน error ของ Postgres ไม่ตรงกับข้อความที่ผู้ใช้พิมพ์
    pub replaced: usize,
}

/// แทน `:hn('...')` / `:nap_no('...')` ด้วย blind index (`index(aad, value)`) ในรูป bytea literal
pub fn expand_helpers(
    sql: &str,
    index: impl Fn(&str, &str) -> Vec<u8>,
) -> Result<Expanded, HelperError> {
    let mut out = String::with_capacity(sql.len());
    let mut replaced = 0;
    let mut i = 0;
    while i < sql.len() {
        let rest = &sql[i..];
        if let Some(len) = skipped_len(sql, i) {
            out.push_str(&sql[i..i + len]);
            i += len;
            continue;
        }
        // `::hn(` คือ cast ไม่ใช่ตัวช่วย
        let after_colon = sql[..i].ends_with(':');
        if let (Some(after), false) = (rest.strip_prefix(':'), after_colon) {
            if let Some((name, aad)) = HELPERS
                .iter()
                .find(|(name, _)| after.starts_with(name) && after[name.len()..].starts_with('('))
            {
                let args_start = i + 1 + name.len() + 1;
                let (value, len) =
                    parse_literal_arg(&sql[args_start..]).ok_or(HelperError::Malformed(name))?;
                out.push_str("'\\x");
                for byte in index(aad, &value) {
                    let _ = write!(out, "{byte:02x}"); // เขียนลง String ไม่มีทางล้ม
                }
                out.push_str("'::bytea");
                replaced += 1;
                i = args_start + len;
                continue;
            }
        }
        let ch = rest.chars().next().unwrap_or_default();
        out.push(ch);
        i += ch.len_utf8().max(1);
    }
    Ok(Expanded { sql: out, replaced })
}

/// `  'ค่า'  )` ต่อจากวงเล็บเปิด → (ค่าที่ unescape `''` แล้ว, ความยาวถึงหลังวงเล็บปิด)
fn parse_literal_arg(text: &str) -> Option<(String, usize)> {
    let start = text.len() - text.trim_start().len();
    let body = text[start..].strip_prefix('\'')?;
    let mut value = String::new();
    let mut chars = body.char_indices();
    let end = loop {
        let (pos, ch) = chars.next()?;
        if ch != '\'' {
            value.push(ch);
            continue;
        }
        if body[pos + 1..].starts_with('\'') {
            value.push('\'');
            chars.next();
            continue;
        }
        break pos + 1;
    };
    let after = &body[end..];
    let close = after.len() - after.trim_start().len();
    after[close..].strip_prefix(')')?;
    // ' + ค่า + ' + ช่องว่าง + )
    Some((value, start + 1 + end + close + 1))
}

/// ส่วนที่ต้องคัดลอกทั้งก้อนโดยไม่ตีความ (string literal, dollar quote, identifier, comment) ที่ตำแหน่ง `i`
/// คืนความยาวเป็น byte (ปิดไม่ครบ = ถึงท้ายข้อความ ให้ Postgres แจ้ง syntax error เอง) — ไม่ใช่ส่วนพวกนี้ = None
fn skipped_len(sql: &str, i: usize) -> Option<usize> {
    let rest = &sql[i..];
    if rest.starts_with("--") {
        return Some(rest.find('\n').unwrap_or(rest.len()));
    }
    if rest.starts_with("/*") {
        return Some(block_comment_len(rest));
    }
    if rest.starts_with('"') {
        return Some(quoted_len(rest, '"', false));
    }
    if rest.starts_with('\'') {
        // E'...' ใช้ backslash escape — ตัว E อยู่ก่อนหน้าและไม่ใช่ส่วนท้ายของชื่ออื่น
        let before = sql[..i].chars().rev();
        let mut before = before.peekable();
        let escape = matches!(before.next(), Some('e' | 'E'))
            && !before
                .peek()
                .is_some_and(|c| c.is_alphanumeric() || *c == '_');
        return Some(quoted_len(rest, '\'', escape));
    }
    if rest.starts_with('$') {
        return dollar_quote_len(rest);
    }
    None
}

fn block_comment_len(rest: &str) -> usize {
    let mut depth = 0;
    let mut j = 0;
    while j < rest.len() {
        if rest[j..].starts_with("/*") {
            depth += 1;
            j += 2;
        } else if rest[j..].starts_with("*/") {
            depth -= 1;
            j += 2;
            if depth == 0 {
                return j;
            }
        } else {
            j += rest[j..].chars().next().map_or(1, char::len_utf8);
        }
    }
    rest.len()
}

/// `'...'` หรือ `"..."` (เครื่องหมายซ้ำสองตัว = ตัวอักษรนั้นหนึ่งตัว) — `backslash` = E-string ที่ `\'` ไม่ปิดข้อความ
fn quoted_len(rest: &str, quote: char, backslash: bool) -> usize {
    let mut chars = rest.char_indices().skip(1);
    while let Some((pos, ch)) = chars.next() {
        if backslash && ch == '\\' {
            chars.next();
        } else if ch == quote {
            if rest[pos + 1..].starts_with(quote) {
                chars.next();
            } else {
                return pos + 1;
            }
        }
    }
    rest.len()
}

/// `$tag$ ... $tag$` (tag ว่างได้) — `$1` (parameter) ไม่ใช่ dollar quote
fn dollar_quote_len(rest: &str) -> Option<usize> {
    let after = &rest[1..];
    let tag_len = after
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .unwrap_or(after.len());
    let tag = &after[..tag_len];
    if tag.starts_with(|c: char| c.is_ascii_digit()) || !after[tag_len..].starts_with('$') {
        return None;
    }
    let delimiter = &rest[..tag_len + 2];
    let body_start = delimiter.len();
    Some(
        rest[body_start..]
            .find(delimiter)
            .map_or(rest.len(), |end| body_start + end + delimiter.len()),
    )
}

/// คำแรกของคำสั่ง (ตัวพิมพ์เล็ก) หลังข้ามช่องว่างและ comment — ไม่มี = ข้อความว่าง
pub fn leading_keyword(sql: &str) -> String {
    let mut i = 0;
    loop {
        let rest = &sql[i..];
        let trimmed = rest.trim_start();
        i += rest.len() - trimmed.len();
        if trimmed.starts_with("--") || trimmed.starts_with("/*") {
            i += skipped_len(sql, i).unwrap_or(trimmed.len());
            continue;
        }
        return trimmed
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .collect::<String>()
            .to_ascii_lowercase();
    }
}

/// คำสั่งที่หน้า SQL ยอมส่งไป Postgres (อ่านอย่างเดียว) ดูจากคำแรก — คำอื่น (insert, update, delete, create, set, copy,
/// call, do, …) ถูกปฏิเสธก่อนรันพร้อมบอกเหตุผล แทนที่จะปล่อยให้กลายเป็น syntax error หลังครอบ LIMIT
/// คำว่าง = ขึ้นต้นด้วยสัญลักษณ์ เช่น `(select …)` ให้ Postgres ตัดสินเอง — ด่านจริงยังเป็นสิทธิ์ของ role + read only
pub fn is_read_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "select" | "with" | "values" | "table" | "explain" | "show" | ""
    )
}

/// คำสั่งที่ครอบด้วย `select * from (…) limit n` ไม่ได้ — explain/show รันตรง ๆ
pub fn runs_unwrapped(keyword: &str) -> bool {
    matches!(keyword, "explain" | "show")
}

/// มี `;` นอก string/identifier/comment ไหม (ตัด `;` ท้ายข้อความออกก่อนเรียก) = ส่งมาหลายคำสั่ง
pub fn has_multiple_statements(sql: &str) -> bool {
    let mut i = 0;
    while i < sql.len() {
        if let Some(len) = skipped_len(sql, i) {
            i += len.max(1);
            continue;
        }
        let ch = sql[i..].chars().next().unwrap_or_default();
        if ch == ';' {
            return true;
        }
        i += ch.len_utf8().max(1);
    }
    false
}

/// ตัด `;` และช่องว่างท้ายข้อความ (ครอบด้วยวงเล็บแล้ว `;` ข้างในจะกลายเป็น syntax error)
pub fn trim_trailing_semicolons(sql: &str) -> &str {
    sql.trim_end_matches(|c: char| c == ';' || c.is_whitespace())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_index(aad: &str, value: &str) -> Vec<u8> {
        // แทน blind index จริงด้วยค่าที่เดาได้ — ไบต์แรกบอกคอลัมน์ ที่เหลือคือค่าที่ trim แล้ว
        let mut out = vec![if aad == AAD_HN { 0xaa } else { 0xbb }];
        out.extend(value.trim().bytes());
        out
    }

    fn expand(sql: &str) -> Result<Expanded, HelperError> {
        expand_helpers(sql, fake_index)
    }

    #[test]
    fn replaces_helpers_with_bytea_literals() {
        let expanded = expand(
            "select uuid from patient where hn_idx = :hn('66471') or nap_no_idx = :nap_no ( 'A1' )",
        )
        .unwrap();
        assert_eq!(
            expanded.sql,
            "select uuid from patient where hn_idx = '\\xaa3636343731'::bytea or nap_no_idx = :nap_no ( 'A1' )"
        );
        assert_eq!(expanded.replaced, 1);

        let spaced = expand(":nap_no(  'A1'  ) x").unwrap();
        assert_eq!(spaced.sql, "'\\xbb4131'::bytea x");
    }

    #[test]
    fn quote_doubling_inside_the_value_is_unescaped() {
        assert_eq!(expand(":hn('a''b')").unwrap().sql, "'\\xaa612762'::bytea");
    }

    #[test]
    fn helpers_inside_strings_comments_and_casts_are_left_alone() {
        for sql in [
            "select ':hn(''1'')'",
            "select 1 -- :hn('1')",
            "select /* :hn('1') /* nested */ :hn('2') */ 1",
            r#"select ":hn('1')" from t"#,
            "select $$ :hn('1') $$",
            "select $body$ :hn('1') $body$",
            "select E'\\' :hn(''1'')'",
            "select x::hn('1')",
        ] {
            let expanded = expand(sql).unwrap();
            assert_eq!(expanded.sql, sql);
            assert_eq!(expanded.replaced, 0, "{sql}");
        }
    }

    #[test]
    fn malformed_helpers_are_rejected_without_echoing_the_value() {
        for sql in [":hn(66471)", ":hn('66471'", ":hn('66471' ,", ":nap_no()"] {
            let err = expand(sql).unwrap_err();
            assert!(!err.to_string().contains("66471"), "{err}");
        }
        assert_eq!(expand(":hn(1)").unwrap_err(), HelperError::Malformed("hn"));
    }

    #[test]
    fn thai_text_and_parameters_survive_unchanged() {
        let sql = "select 'สมชาย', $1, \"ชื่อ\" from t where a = :hn('ก')";
        let expanded = expand(sql).unwrap();
        assert!(expanded
            .sql
            .starts_with("select 'สมชาย', $1, \"ชื่อ\" from t where a = "));
        assert!(expanded.sql.ends_with("'::bytea"));
    }

    #[test]
    fn leading_keyword_skips_comments_and_whitespace() {
        assert_eq!(leading_keyword("  SELECT 1"), "select");
        assert_eq!(leading_keyword("-- x\n/* y */ explain select 1"), "explain");
        assert_eq!(leading_keyword("(select 1)"), "");
        assert_eq!(leading_keyword(""), "");
        assert!(runs_unwrapped("explain") && runs_unwrapped("show"));
        assert!(!runs_unwrapped("select") && !runs_unwrapped("with"));
    }

    #[test]
    fn trailing_semicolons_are_trimmed() {
        assert_eq!(trim_trailing_semicolons("select 1 ;\n ;"), "select 1");
        assert_eq!(trim_trailing_semicolons("select ';'"), "select ';'");
    }

    #[test]
    fn only_read_statements_pass_the_keyword_check() {
        for sql in [
            "select 1",
            "  -- comment
WITH x as (select 1) select * from x",
            "values (1)",
            "table prename",
            "explain select 1",
            "show timezone",
            "(select 1)",
        ] {
            assert!(is_read_keyword(&leading_keyword(sql)), "{sql}");
        }
        for sql in [
            "insert into x values (1)",
            "/* x */ UPDATE x set a = 1",
            "delete from x",
            "create table x (a int)",
            "drop table x",
            "set role postgres",
            "copy x to program 'id'",
            "do $$ begin end $$",
            "call p()",
            "truncate x",
        ] {
            assert!(!is_read_keyword(&leading_keyword(sql)), "{sql}");
        }
    }

    #[test]
    fn finds_semicolons_between_statements_only() {
        assert!(has_multiple_statements("select 1; delete from x"));
        assert!(has_multiple_statements("select 1;select 2"));
        assert!(!has_multiple_statements(
            "select 'a;b', \"c;d\" from t -- e;f"
        ));
        assert!(!has_multiple_statements("select $$x;y$$, /* ; */ 1"));
        assert!(!has_multiple_statements("select 1"));
    }
}
