use serde_json::{Map, Value};
use telbot_cf_worker::types::markup::{
    InlineKeyboardButton, InlineKeyboardButtonKind, InlineKeyboardMarkup, ParseMode,
};
use telbot_cf_worker::types::message::{SendMessage, SendPhoto};
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

pub fn tier_to_name(level: u64) -> Option<String> {
    match level {
        0 => Some("Unranked".to_string()),
        1..=29 => Some(format!(
            "{} {}",
            ["Bronze", "Silver", "Gold", "Platinum", "Diamond", "Ruby"][(level as usize - 1) / 5],
            ["V", "IV", "III", "II", "I"][(level as usize - 1) % 5]
        )),
        30 => Some("Master".to_string()),
        _ => None,
    }
}

pub fn class_to_name(class: u64, decoration: &str) -> Option<String> {
    let decoration = match decoration {
        "none" => Some(""),
        "silver" => Some("+"),
        "gold" => Some("++"),
        _ => None,
    };
    decoration.map(|decoration| format!("{}{}", class, decoration))
}

pub fn search_problem_to_query(result: &[Map<String, Value>]) -> Vec<InlineQueryResult> {
    result
        .iter()
        .map(|obj| {
            let id = extract_u64_or_na(obj, "problemId");
            let title = extract_str_or_na(obj, "titleKo");
            let level = obj
                .get("level")
                .and_then(Value::as_u64)
                .and_then(level_to_name)
                .unwrap_or_else(|| "N/A".to_string());
            let partial = obj
                .get("isPartial")
                .and_then(Value::as_bool)
                .map_or("", |is_partial| {
                    if is_partial {
                        " [부분 점수 / 서브태스크]"
                    } else {
                        ""
                    }
                });
            let solvable =
                obj.get("isSolvable")
                    .and_then(Value::as_bool)
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

pub fn problem_show_to_message(chat_id: i64, result: &[Map<String, Value>]) -> SendMessage {
    let mut text: String = result
        .iter()
        .map(|problem| {
            let id = extract_u64_or_na(problem, "problemId");
            let title = extract_str_or_na(problem, "titleKo");
            let level = problem
                .get("level")
                .and_then(Value::as_u64)
                .and_then(level_to_name)
                .unwrap_or_else(|| "N/A".to_string());
            format!(
                "[{} \\- \\#{} {}](https://boj.kr/{1})\n",
                level,
                id,
                escape_markdown_v2(title),
            )
        })
        .collect();
    text.pop();
    SendMessage::new(chat_id, text)
        .disable_web_page_preview()
        .with_parse_mode(ParseMode::MarkdownV2)
}

pub fn user_show_to_message(chat_id: i64, result: Map<String, Value>) -> SendPhoto {
    let handle = extract_str_or_na(&result, "handle");
    let rank = result
        .get("rank")
        .and_then(Value::as_u64)
        .map(|rank| format!(" \\({}위\\)", rank))
        .unwrap_or_default();
    let tier = result
        .get("tier")
        .and_then(Value::as_u64)
        .and_then(tier_to_name)
        .unwrap_or_else(|| "N/A".to_string());
    let class = result.get("class").and_then(Value::as_u64);
    let class_deco = extract_str_or_na(&result, "classDecoration");
    let class_name = class
        .and_then(|class| class_to_name(class, class_deco))
        .unwrap_or_else(|| "N/A".to_string());
    let rating = extract_u64_or_na(&result, "rating");
    let problem_rating = extract_u64_or_na(&result, "rating");
    let class_rating = extract_u64_or_na(&result, "ratingByClass");
    let solve_rating = extract_u64_or_na(&result, "ratingBySolvedCount");
    let vote_rating = extract_u64_or_na(&result, "ratingByVoteCount");
    let bio = extract_str_or_na(&result, "bio");
    let profile_image = result
        .get("profileImageUrl")
        .and_then(Value::as_str)
        .unwrap_or("https://static.solved.ac/misc/360x360/default_profile.png");

    let text = format!(
        "*{handle}*{rank}\n\
        {tier} 클래스 {class}\n\
        레이팅 {rating} \\(난이도 {prating}/클래스 {crating}/풀이 {srating}/기여 {vrating}\\)\n\
        \n\
        {bio}\n",
        handle = handle,
        rank = rank,
        tier = tier,
        class = class_name,
        rating = rating,
        prating = problem_rating,
        crating = class_rating,
        srating = solve_rating,
        vrating = vote_rating,
        bio = escape_markdown_v2(bio),
    );

    let keyboard = InlineKeyboardMarkup {
        inline_keyboard: vec![
            vec![InlineKeyboardButton {
                text: "solved.ac 프로필".to_string(),
                kind: InlineKeyboardButtonKind::Url {
                    url: format!("https://solved.ac/profile/{}", handle),
                },
            }],
            vec![InlineKeyboardButton {
                text: "acmicpc.net 프로필".to_string(),
                kind: InlineKeyboardButtonKind::Url {
                    url: format!("https://acmicpc.net/user/{}", handle),
                },
            }],
        ],
    };

    SendPhoto::new(chat_id, profile_image)
        .with_caption(text)
        .with_parse_mode(ParseMode::MarkdownV2)
        .with_reply_markup(keyboard)
}

fn extract_u64_or_na(map: &Map<String, Value>, key: &str) -> String {
    map.get(key)
        .and_then(Value::as_u64)
        .as_ref()
        .map_or_else(|| "N/A".to_string(), u64::to_string)
}

fn extract_str_or_na<'a>(map: &'a Map<String, Value>, key: &str) -> &'a str {
    map.get(key).and_then(Value::as_str).unwrap_or("N/A")
}
