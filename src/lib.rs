use db::ProfileImages;
use serde_json::Value;
use std::result::Result;
use telbot_cf_worker::types::markup::ParseMode;
use telbot_cf_worker::types::message::{Message, MessageKind, SendMessage};
use telbot_cf_worker::types::query::AnswerInlineQuery;
use telbot_cf_worker::types::update::*;
use telbot_cf_worker::Api;
use worker::js_sys::{Number, RegExp};
use worker::*;

use crate::command::Command;
use crate::db::RatingAlarms;

mod command;
mod db;
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
    let rating_endpoint = format!("/rating/{}", token);
    router
        .post_async(&bot_endpoint, |req, ctx| async {
            if let Err(e) = handle_request(req, ctx).await {
                web_sys::console::error_1(&e.to_string().into());
            }
            Response::empty()
        })
        .get_async(&rating_endpoint, |_, ctx| async move {
            let ratings = RatingAlarms::setup(ctx.kv("RATING_ALARMS")?);
            for subscriber in ratings.all_subscribers().await? {
                if let Some(rating) = ratings.get_subscription(subscriber).await? {
                    if let Some(user_info) = solved::user_show(&rating.target).await? {
                        let new_rating = user_info.get("rating").unwrap().as_u64().unwrap();
                        if rating.rating != new_rating {
                            ctx.data()
                                .send_json(&formatter::rating_update_to_message(
                                    subscriber,
                                    &rating.target,
                                    rating.rating,
                                    user_info,
                                ))
                                .await
                                .map_err(convert_error)?;
                            ratings
                                .set_subscription(subscriber, rating.target, new_rating)
                                .await?;
                        }
                    }
                }
            }
            Response::empty()
        })
        .run(req, env)
        .await
}

async fn handle_request(mut req: Request, ctx: RouteContext<Api>) -> worker::Result<()> {
    let update = req.json::<Update>().await?;
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
                answer_query = answer_query.with_next_offset((page * 2 + parity).to_string());
            }
            ctx.data()
                .send_json(&answer_query)
                .await
                .map_err(convert_error)?;
        }
        UpdateKind::Message { message } => {
            if let Some(text) = message.text() {
                let command = Command::new(text);
                let mut args = command.args();
                match command.label {
                    "/problem" => match args.map(str::parse).collect::<Result<Vec<_>, _>>() {
                        Ok(id_list) if !id_list.is_empty() => {
                            let problems = solved::problem_lookup(&id_list).await?;
                            if problems.is_empty() {
                                let req =
                                    SendMessage::new(message.chat.id, "????????? ?????? ??? ????????????.");
                                ctx.data().send_json(&req).await.map_err(convert_error)?;
                            } else {
                                let req =
                                    formatter::problem_show_to_message(message.chat.id, &problems);
                                ctx.data().send_json(&req).await.map_err(convert_error)?;
                            }
                        }
                        _ => {
                            let req = SendMessage::new(
                                message.chat.id,
                                "?????????: /problem <????????????1> <????????????2> <...>",
                            );
                            ctx.data().send_json(&req).await.map_err(convert_error)?;
                        }
                    },
                    "/user" => {
                        if let Some(handle) = args.next() {
                            let user = solved::user_show(handle).await?;
                            if let Some(user) = user {
                                let images = ProfileImages::setup(ctx.kv("PROFILE_IMAGES")?);
                                let profile = images.get_id(handle).await?;
                                let req = formatter::user_show_to_message(
                                    message.chat.id,
                                    user,
                                    profile.clone().map(Into::into),
                                )
                                .await?;
                                let message =
                                    ctx.data().send_file(&req).await.map_err(convert_error)?;
                                if let MessageKind::Document { document, .. } = message.kind {
                                    images.set_id(handle, &document.file_id).await?;
                                }
                            } else {
                                let req =
                                    SendMessage::new(message.chat.id, "???????????? ?????? ??? ????????????.");
                                ctx.data().send_json(&req).await.map_err(convert_error)?;
                            }
                        } else {
                            let help =
                                SendMessage::new(message.chat.id, "?????????: /user <????????????>");
                            ctx.data().send_json(&help).await.map_err(convert_error)?;
                        }
                    }
                    "/get" => {
                        if let Some(reply_to) =
                            message.reply_to_message.as_deref().and_then(Message::text)
                        {
                            let regex = RegExp::new(r"(\d+)????", "g");
                            let mut problems = vec![];
                            while let Some(captures) = regex.exec(reply_to) {
                                let number = Number::from(captures.get(1)).value_of() as u32;
                                problems.push(number);
                            }
                            if !problems.is_empty() {
                                let problems = solved::problem_lookup(&problems).await?;
                                let request =
                                    formatter::problem_show_to_message(message.chat.id, &problems)
                                        .reply_to(message.message_id);
                                ctx.data()
                                    .send_json(&request)
                                    .await
                                    .map_err(convert_error)?;
                            }
                        }
                    }
                    "/subscribe" => match args.next() {
                        Some("ratings") => {
                            if let Some(handle) = args.next() {
                                let ratings = RatingAlarms::setup(ctx.kv("RATING_ALARMS")?);
                                if let Some(user) = solved::user_show(handle).await? {
                                    ratings
                                        .set_subscription(
                                            message.chat.id,
                                            handle,
                                            user.get("rating").and_then(Value::as_u64).unwrap(),
                                        )
                                        .await?;

                                    let success = SendMessage::new(
                                        message.chat.id,
                                        format!("*{}*?????? ????????? ????????? ??????????????????\\.", handle),
                                    )
                                    .with_parse_mode(ParseMode::MarkdownV2);
                                    ctx.data()
                                        .send_json(&success)
                                        .await
                                        .map_err(convert_error)?;
                                } else {
                                    let error = SendMessage::new(
                                        message.chat.id,
                                        "???????????? ?????? ??? ????????????.",
                                    );
                                    ctx.data().send_json(&error).await.map_err(convert_error)?;
                                }
                            } else {
                                let help = SendMessage::new(
                                    message.chat.id,
                                    "?????????: /subscribe ratings <????????????>",
                                );
                                ctx.data().send_json(&help).await.map_err(convert_error)?;
                            }
                        }
                        _ => {
                            let help = SendMessage::new(
                                message.chat.id,
                                "?????????: /subscribe ratings <????????????>",
                            );
                            ctx.data().send_json(&help).await.map_err(convert_error)?;
                        }
                    },
                    "/unsubscribe" => match args.next() {
                        Some("ratings") => {
                            let ratings = RatingAlarms::setup(ctx.kv("RATING_ALARMS")?);
                            ratings.unsubscribe(message.chat.id).await?;
                            let success = SendMessage::new(
                                message.chat.id,
                                "????????? ?????? ????????? ?????????????????????.",
                            );
                            ctx.data()
                                .send_json(&success)
                                .await
                                .map_err(convert_error)?;
                        }
                        _ => {
                            let help =
                                SendMessage::new(message.chat.id, "?????????: /unsubscribe ratings");
                            ctx.data().send_json(&help).await.map_err(convert_error)?;
                        }
                    },
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn convert_error(error: telbot_cf_worker::Error) -> worker::Error {
    match error {
        telbot_cf_worker::Error::TelegramError(e) => {
            worker::Error::RustError(format!("Telegram Error: {}", e.description))
        }
        telbot_cf_worker::Error::Worker(e) => e,
        telbot_cf_worker::Error::Io(e) => worker::Error::RustError(format!("IO Error: {}", e)),
    }
}
