use serde_json::{Map, Value};
use telbot_cf_worker::types::markup::ParseMode;
use telbot_cf_worker::types::query::{
    InlineQueryResult, InlineQueryResultKind, InputMessageContent,
};
use worker::js_sys::{JsString, RegExp};

pub fn escape_markdown_v2(s: &str) -> String {
    let regex = RegExp::new(r"[_*\[\]()~`>#+-=|\{\}\.!]", "g");
    JsString::from(s).replace_by_pattern(&regex, r"\$&").into()
}

pub fn level_to_name(level: u64) -> Option<String> {
    match level {
        0 => Some("Unrated".to_string()),
        1..=30 => Some(format!(
            "{} {}",
            ["Bronze", "Silver", "Gold", "Platinum", "Diamond", "Ruby"][(level as usize - 1) / 5],
            ["V", "IV", "III", "II", "I"][(level as usize - 1) % 5]
        )),
        _ => None,
    }
}

pub fn search_problem_to_query(result: &[Map<String, Value>]) -> Vec<InlineQueryResult> {
    result
        .iter()
        .map(|obj| {
            let id = obj
                .get("problemId")
                .and_then(|obj| obj.as_u64())
                .map(|u| u.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            let title = obj
                .get("titleKo")
                .and_then(|obj| obj.as_str())
                .unwrap_or("N/A");
            let level = obj
                .get("level")
                .and_then(|obj| obj.as_u64())
                .and_then(level_to_name)
                .unwrap_or_else(|| "N/A".to_string());
            let partial =
                obj.get("isPartial")
                    .and_then(|obj| obj.as_bool())
                    .map_or("", |is_partial| {
                        if is_partial {
                            " [부분 점수 / 서브태스크]"
                        } else {
                            ""
                        }
                    });
            let solvable =
                obj.get("isSolvable")
                    .and_then(|obj| obj.as_bool())
                    .map_or("", |is_solvable| {
                        if is_solvable {
                            ""
                        } else {
                            " \\(채점 준비중\\)"
                        }
                    });
            let content = format!(
                "[{} \\- \\#{} {}](https://boj.kr/{1})\n{}{}",
                level,
                id,
                escape_markdown_v2(title),
                partial,
                solvable
            );

            InlineQueryResultKind::Article {
                title: title.to_string(),
                description: Some(level),
                input_message_content: InputMessageContent::Text {
                    message_text: content,
                    disable_web_page_preview: Some(false),
                    entities: None,
                    parse_mode: Some(ParseMode::MarkdownV2),
                },
                url: None,
                hide_url: None,
                thumb_url: None,
                thumb_width: None,
                thumb_height: None,
            }
            .with_id(format!("SPTQ{}", id))
        })
        .collect()
}
