use crate::ApiContext;
use common::structs::{JoinRequestSummary, ReportSummary};
use yew::prelude::*;

/// The moderation page: the report inbox, plus the join-request queue for site moderators.
///
/// Shows only what the applicant wrote and when. There is deliberately nothing here that
/// could later be matched against a member — no hash, no subject, no account identifier —
/// so reviewing someone does not let you recognise their posts afterwards.
#[function_component]
pub fn Moderation() -> Html {
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let me = use_context::<crate::MeContext>().unwrap_or_default();
    let requests = use_state(|| None::<Result<Vec<JoinRequestSummary>, AttrValue>>);
    let reports = use_state(|| None::<Result<Vec<ReportSummary>, AttrValue>>);
    let reload = use_state(|| 0u32);

    {
        let api_ctx = api_ctx.clone();
        let reports = reports.clone();
        use_effect_with(*reload, move |_| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                reports.set(Some(Err(AttrValue::Static("No connection to the server."))));
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                match api.list_reports().await {
                    Ok(v) => reports.set(Some(Ok(v))),
                    Err(e) => reports.set(Some(Err(AttrValue::from(e.to_string())))),
                }
            });
        });
    }

    let resolve = {
        let api_ctx = api_ctx.clone();
        let reload = reload.clone();
        Callback::from(move |id: String| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                return;
            };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.resolve_report(&id).await {
                    gloo::console::error!(format!("could not resolve report: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    {
        let api_ctx = api_ctx.clone();
        let requests = requests.clone();
        use_effect_with(*reload, move |_| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                requests.set(Some(Err(AttrValue::Static("No connection to the server."))));
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                match api.list_access_requests().await {
                    Ok(v) => requests.set(Some(Ok(v))),
                    Err(e) => requests.set(Some(Err(AttrValue::from(e.to_string())))),
                }
            });
        });
    }

    let decide = {
        let api_ctx = api_ctx.clone();
        let reload = reload.clone();
        Callback::from(move |(id, action): (String, &'static str)| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                return;
            };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.decide_access_request(&id, action).await {
                    gloo::console::error!(format!("could not {action} request: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    let body = match &*requests {
        None => html! { <crate::components::Spinner /> },
        Some(Err(e)) => html! { <p class="join-error">{e.clone()}</p> },
        Some(Ok(list)) if list.is_empty() => {
            html! { <p class="moderation-empty">{"Nothing waiting."}</p> }
        }
        Some(Ok(list)) => html! {
            <div class="moderation-list">
                { for list.iter().map(|r| {
                    let id_approve = r.id.clone();
                    let id_decline = r.id.clone();
                    let decide_a = decide.clone();
                    let decide_d = decide.clone();
                    html! {
                        <div class="moderation-item" key={r.id.clone()}>
                            <p class="moderation-disclosure">{ r.disclosure.clone() }</p>
                            <p class="moderation-meta">
                                { r.created_at.format("%Y-%m-%d %H:%M UTC").to_string() }
                            </p>
                            <div class="moderation-actions">
                                <button
                                    class="join-button"
                                    onclick={Callback::from(move |_| decide_a.emit((id_approve.clone(), "approve")))}
                                >{"Approve"}</button>
                                <button
                                    class="join-button join-button-danger"
                                    onclick={Callback::from(move |_| decide_d.emit((id_decline.clone(), "decline")))}
                                >{"Decline"}</button>
                            </div>
                        </div>
                    }
                }) }
            </div>
        },
    };

    let reports_body = match &*reports {
        None => html! { <crate::components::Spinner /> },
        Some(Err(e)) => html! { <p class="join-error">{e.clone()}</p> },
        Some(Ok(list)) if list.is_empty() => {
            html! { <p class="moderation-empty">{"No open reports."}</p> }
        }
        Some(Ok(list)) => html! {
            <div class="moderation-list">
                { for list.iter().map(|r| {
                    let id = r.id.clone();
                    let resolve = resolve.clone();
                    let link = format!("/{}/thread/{}", r.board_discriminator, r.post_number);
                    html! {
                        <div class="moderation-item" key={r.id.clone()}>
                            <p class="moderation-disclosure">{ r.reason.clone() }</p>
                            <p class="moderation-meta">
                                <a href={link}>
                                    { format!("/{}/ post {}", r.board_discriminator, r.post_number) }
                                </a>
                                { format!(" — {}", r.created_at.format("%Y-%m-%d %H:%M UTC")) }
                            </p>
                            <div class="moderation-actions">
                                <button
                                    class="join-button"
                                    onclick={Callback::from(move |_| resolve.emit(id.clone()))}
                                >{"Mark handled"}</button>
                            </div>
                        </div>
                    }
                }) }
            </div>
        },
    };

    html! {
        <div class="join-page moderation-page">
            <h1>{"Reports"}</h1>
            <p class="moderation-note">
                {"Flagged posts on boards you moderate. Reports carry no reporter."}
            </p>
            { reports_body }

            if !me.can_moderate_site() {
                <></>
            } else {
            <h1>{"Join requests"}</h1>
            <p class="moderation-note">
                {"Approving does not create the account immediately — it is created when that \
                  person next signs in."}
            </p>
            { body }
            }
        </div>
    }
}
