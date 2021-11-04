use telbot_cf_worker::types::query::AnswerInlineQuery;
use telbot_cf_worker::types::update::*;
use worker::*;

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
pub async fn main(req: Request, env: Env) -> Result<Response> {
    log_request(&req);
    utils::set_panic_hook();
    let token = env.secret("BOT_TOKEN")?.to_string();
    let api = telbot_cf_worker::Api::new(&token);
    let router = Router::with_data(api);
    let bot_endpoint = format!("/{}", token);
    router
        .post_async(&bot_endpoint, |mut req, ctx| async move {
            let update = req.json::<Update>().await.unwrap();
            #[allow(clippy::single_match)]
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
                _ => {}
            }
            Response::empty()
        })
        .run(req, env)
        .await
}
