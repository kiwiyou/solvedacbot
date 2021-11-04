use std::result::Result;
use telbot_cf_worker::types::message::SendMessage;
use telbot_cf_worker::types::query::AnswerInlineQuery;
use telbot_cf_worker::types::update::*;
use worker::*;

use crate::command::Command;

mod command;
mod formatter;
mod solved;
mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or_else(|| "unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env) -> worker::Result<Response> {
    log_request(&req);
    utils::set_panic_hook();
    let token = env.secret("BOT_TOKEN")?.to_string();
    let api = telbot_cf_worker::Api::new(&token);
    let router = Router::with_data(api);
    let bot_endpoint = format!("/{}", token);
    router
        .post_async(&bot_endpoint, |mut req, ctx| async move {
            let update = req.json::<Update>().await.unwrap();
            match update.kind {
                UpdateKind::InlineQuery { inline_query } => {
                    let (page, parity) = match inline_query.offset.parse::<u32>() {
                        Ok(page) if page > 0 => ((page + 1) / 2, (page + 1) % 2),
                        _ => (1, 0),
                    };
                    let mut result = solved::search_problem(&inline_query.query, page).await?;
                    let (result, has_next) = if parity == 1 {
                        if result.len() > 50 {
                            (&result[50..], result.len() >= 100)
                        } else {
                            ([].as_ref(), false)
                        }
                    } else {
                        result.truncate(50);
                        (&result[..], result.len() >= 50)
                    };
                    let response = formatter::search_problem_to_query(result);
                    let mut answer_query = AnswerInlineQuery::new(inline_query.id, response);
                    if has_next {
                        answer_query =
                            answer_query.with_next_offset((page * 2 + parity).to_string());
                    }
                    ctx.data().send_json(&answer_query).await.unwrap();
                }
                UpdateKind::Message { message } => {
                    if let Some(text) = message.text() {
                        let command = Command::new(text);
                        let mut args = command.args();
                        match command.label {
                            "/problem" => {
                                match args.map(str::parse).collect::<Result<Vec<_>, _>>() {
                                    Ok(id_list) if !id_list.is_empty() => {
                                        let problems = solved::problem_lookup(&id_list).await?;
                                        if problems.is_empty() {
                                            let req = SendMessage::new(
                                                message.chat.id,
                                                "문제를 찾을 수 없습니다.",
                                            );
                                            ctx.data().send_json(&req).await.unwrap();
                                        } else {
                                            let req = formatter::problem_show_to_message(
                                                message.chat.id,
                                                &problems,
                                            );
                                            ctx.data().send_json(&req).await.unwrap();
                                        }
                                    }
                                    _ => {
                                        let req = SendMessage::new(
                                            message.chat.id,
                                            "사용법: /problem <문제번호1> <문제번호2> <...>",
                                        );
                                        ctx.data().send_json(&req).await.unwrap();
                                    }
                                }
                            }
                            "/user" => {
                                if let Some(handle) = args.next() {
                                    let user = solved::user_show(handle).await?;
                                    if let Some(user) = user {
                                        let req =
                                            formatter::user_show_to_message(message.chat.id, user);
                                        ctx.data().send_file(&req).await.unwrap();
                                    } else {
                                        let req = SendMessage::new(
                                            message.chat.id,
                                            "사용자를 찾을 수 없습니다.",
                                        );
                                        ctx.data().send_json(&req).await.unwrap();
                                    }
                                } else {
                                    let help = SendMessage::new(
                                        message.chat.id,
                                        "사용법: /user <사용자명>",
                                    );
                                    ctx.data().send_json(&help).await.unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
            Response::empty()
        })
        .run(req, env)
        .await
}
