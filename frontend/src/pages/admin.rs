use crate::{ApiContext, MeContext};
use common::structs::{JoinRequestSummary, LogEntry, ModeratorGrant, ReportSummary, SafeBoard};
use yew::prelude::*;

/// The admin panel.
///
/// One page for every moderation tier. Which tabs exist is decided by the viewer's scope, so a
/// board moderator and a site admin render the same component tree with different sections
/// present — rather than two pages that drift apart.
///
/// Note what is absent: there is no member list anywhere, and no way to go from anything shown
/// here to a person. Moderator status is managed as *grants* (issue a code, revoke a grant),
/// because members cannot be enumerated by design.
#[function_component]
pub fn Admin() -> Html {
    let me = use_context::<MeContext>().unwrap_or_default();
    let tab = use_state(|| Tab::Reports);

    if !me.can_moderate_anything() {
        return html! {
            <div class="join-page">
                <h1>{"Nothing here"}</h1>
                <p>{"You do not moderate anything."}</p>
            </div>
        };
    }

    let is_site = me.can_moderate_site();

    let mut tabs = vec![(Tab::Reports, "Reports")];
    if is_site {
        tabs.push((Tab::Requests, "Join requests"));
    }
    // Board moderators get a Boards tab too, showing only the boards they moderate.
    tabs.push((Tab::Boards, "Boards"));
    if is_site {
        tabs.push((Tab::Moderators, "Moderators"));
    }
    tabs.push((Tab::Log, "Audit log"));

    html! {
        <div class="admin-page">
            <h1>{"Moderation"}</h1>
            <div class="admin-tabs">
                { for tabs.into_iter().map(|(t, label)| {
                    let active = *tab == t;
                    let set = { let tab = tab.clone(); Callback::from(move |_| tab.set(t)) };
                    html! {
                        <button
                            class={classes!("admin-tab", active.then_some("admin-tab-active"))}
                            onclick={set}
                        >{label}</button>
                    }
                }) }
            </div>
            <div class="admin-body">
                {
                    match *tab {
                        Tab::Reports => html! { <ReportsTab /> },
                        Tab::Requests => html! { <RequestsTab /> },
                        Tab::Boards => html! { <BoardsTab /> },
                        Tab::Moderators => html! { <ModeratorsTab /> },
                        Tab::Log => html! { <LogTab /> },
                    }
                }
            </div>
        </div>
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Reports,
    Requests,
    Boards,
    Moderators,
    Log,
}

/// Fetch-with-reload shared by every tab.
#[hook]
fn use_listing<T, F, Fut>(reload: u32, fetch: F) -> UseStateHandle<Option<Result<Vec<T>, AttrValue>>>
where
    T: 'static,
    F: Fn(std::sync::Arc<crate::api::Api>) -> Fut + 'static,
    Fut: std::future::Future<Output = Result<Vec<T>, crate::api::ApiError>> + 'static,
{
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let state = use_state(|| None::<Result<Vec<T>, AttrValue>>);
    {
        let state = state.clone();
        use_effect_with(reload, move |_| {
            let Some(api) = api_ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned() else {
                state.set(Some(Err(AttrValue::Static("No connection to the server."))));
                return;
            };
            wasm_bindgen_futures::spawn_local(async move {
                match fetch(api).await {
                    Ok(v) => state.set(Some(Ok(v))),
                    Err(e) => state.set(Some(Err(AttrValue::from(e.to_string())))),
                }
            });
        });
    }
    state
}

fn listing<T>(
    state: &Option<Result<Vec<T>, AttrValue>>,
    empty: &'static str,
    row: impl Fn(&T) -> Html,
) -> Html {
    match state {
        None => html! { <crate::components::Spinner /> },
        Some(Err(e)) => html! { <p class="join-error">{e.clone()}</p> },
        Some(Ok(list)) if list.is_empty() => html! { <p class="moderation-empty">{empty}</p> },
        Some(Ok(list)) => html! { <div class="moderation-list">{ for list.iter().map(row) }</div> },
    }
}

fn api_of(ctx: &Option<ApiContext>) -> Option<std::sync::Arc<crate::api::Api>> {
    ctx.as_ref().and_then(|c| c.api.as_ref().ok()).cloned()
}

#[function_component]
fn ReportsTab() -> Html {
    let reload = use_state(|| 0u32);
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let reports = use_listing::<ReportSummary, _, _>(*reload, |api| async move {
        api.list_reports().await
    });

    let resolve = {
        let api_ctx = api_ctx.clone();
        let reload = reload.clone();
        Callback::from(move |id: String| {
            let Some(api) = api_of(&api_ctx) else { return };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.resolve_report(&id).await {
                    gloo::console::error!(format!("resolve failed: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    html! {
        <>
            <p class="moderation-note">
                {"Flagged posts on boards you moderate. Reports carry no reporter."}
            </p>
            { listing(&reports, "No open reports.", |r: &ReportSummary| {
                let id = r.id.clone();
                let resolve = resolve.clone();
                html! {
                    <div class="moderation-item" key={r.id.clone()}>
                        <p class="moderation-disclosure">{ r.reason.clone() }</p>
                        <p class="moderation-meta">
                            <a href={format!("/{}/thread/{}", r.board_discriminator, r.post_number)}>
                                { format!("/{}/ post {}", r.board_discriminator, r.post_number) }
                            </a>
                            { format!(" — {}", r.created_at.format("%Y-%m-%d %H:%M UTC")) }
                        </p>
                        <div class="moderation-actions">
                            <button class="join-button"
                                onclick={Callback::from(move |_| resolve.emit(id.clone()))}>
                                {"Mark handled"}
                            </button>
                        </div>
                    </div>
                }
            }) }
        </>
    }
}

#[function_component]
fn RequestsTab() -> Html {
    let reload = use_state(|| 0u32);
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let requests = use_listing::<JoinRequestSummary, _, _>(*reload, |api| async move {
        api.list_access_requests().await
    });

    let decide = {
        let api_ctx = api_ctx.clone();
        let reload = reload.clone();
        Callback::from(move |(id, action): (String, &'static str)| {
            let Some(api) = api_of(&api_ctx) else { return };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.decide_access_request(&id, action).await {
                    gloo::console::error!(format!("{action} failed: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    html! {
        <>
            <p class="moderation-note">
                {"Approving does not create the account immediately — it is created when that \
                  person next signs in."}
            </p>
            { listing(&requests, "Nothing waiting.", |r: &JoinRequestSummary| {
                let (a, d) = (r.id.clone(), r.id.clone());
                let (da, dd) = (decide.clone(), decide.clone());
                html! {
                    <div class="moderation-item" key={r.id.clone()}>
                        <p class="moderation-disclosure">{ r.disclosure.clone() }</p>
                        <p class="moderation-meta">
                            { r.created_at.format("%Y-%m-%d %H:%M UTC").to_string() }
                        </p>
                        <div class="moderation-actions">
                            <button class="join-button"
                                onclick={Callback::from(move |_| da.emit((a.clone(), "approve")))}>
                                {"Approve"}
                            </button>
                            <button class="join-button join-button-danger"
                                onclick={Callback::from(move |_| dd.emit((d.clone(), "decline")))}>
                                {"Decline"}
                            </button>
                        </div>
                    </div>
                }
            }) }
        </>
    }
}

#[function_component]
fn BoardsTab() -> Html {
    let reload = use_state(|| 0u32);
    let me = use_context::<MeContext>().unwrap_or_default();
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let boards = use_listing::<SafeBoard, _, _>(*reload, |api| async move {
        api.get_boards(true).await
    });
    let name = use_state(String::new);
    let disc = use_state(String::new);

    let create = {
        let (api_ctx, reload, name, disc) = (api_ctx.clone(), reload.clone(), name.clone(), disc.clone());
        Callback::from(move |_: MouseEvent| {
            let Some(api) = api_of(&api_ctx) else { return };
            let (n, d) = (name.trim().to_owned(), disc.trim().to_owned());
            if n.is_empty() || d.is_empty() {
                return;
            }
            let (reload, name, disc) = (reload.clone(), name.clone(), disc.clone());
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.create_board(n, d).await {
                    gloo::console::error!(format!("create board failed: {}", *e));
                }
                name.set(String::new());
                disc.set(String::new());
                reload.set(*reload + 1);
            });
        })
    };

    let delete = {
        let (api_ctx, reload) = (api_ctx.clone(), reload.clone());
        Callback::from(move |d: String| {
            // Irreversible and takes every thread, post and file with it.
            let confirmed = web_sys::window()
                .and_then(|w| {
                    w.confirm_with_message(&format!(
                        "Delete /{d}/ permanently?\n\nEvery thread, post and file on it goes \
                         too. This cannot be undone."
                    ))
                    .ok()
                })
                .unwrap_or(false);
            if !confirmed {
                return;
            }
            let Some(api) = api_of(&api_ctx) else { return };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.delete_board(&d).await {
                    gloo::console::error!(format!("delete failed: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    let toggle_private = {
        let (api_ctx, reload) = (api_ctx.clone(), reload.clone());
        Callback::from(move |(d, private): (String, bool)| {
            let Some(api) = api_of(&api_ctx) else { return };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let update = common::structs::BoardUpdate { name: None, private: Some(private) };
                if let Err(e) = api.update_board(&d, update).await {
                    gloo::console::error!(format!("update failed: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    let on_name = { let name = name.clone(); Callback::from(move |e: InputEvent| {
        if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() { name.set(el.value()); }
    })};
    let on_disc = { let disc = disc.clone(); Callback::from(move |e: InputEvent| {
        if let Some(el) = e.target_dyn_into::<web_sys::HtmlInputElement>() { disc.set(el.value()); }
    })};

    let is_site = me.can_moderate_site();
    let is_admin = me.is_admin();
    let mine = me.moderated_boards();

    // Site roles see every board; a board moderator sees only their own.
    let visible = boards.as_ref().map(|res| {
        res.as_ref().map(|list| {
            list.iter()
                .filter(|b| is_site || mine.contains(&b.discriminator))
                .cloned()
                .collect::<Vec<_>>()
        }).map_err(|e| e.clone())
    });

    html! {
        <>
            if is_site {
                <div class="admin-create-board">
                    <input class="admin-input" placeholder="Name" value={(*name).clone()} oninput={on_name} />
                    <input class="admin-input admin-input-short" placeholder="/d/" value={(*disc).clone()} oninput={on_disc} />
                    <button class="join-button" onclick={create}>{"Create board"}</button>
                </div>
            }
            <p class="moderation-note">
                {"A board's short name cannot be changed: it appears in every URL and in access \
                  grants, so renaming it would orphan them."}
            </p>
            { listing(&visible, "No boards you can manage.", |b: &SafeBoard| {
                let d = b.discriminator.clone();
                let del_d = b.discriminator.clone();
                let next = !b.private;
                let toggle = toggle_private.clone();
                let delete = delete.clone();
                html! {
                    <div class="moderation-item" key={b.discriminator.clone()}>
                        <p class="moderation-disclosure">
                            { format!("/{}/ — {}", b.discriminator, b.name) }
                        </p>
                        <p class="moderation-meta">
                            { if b.private { "Private — invite only" } else { "Public to members" } }
                        </p>
                        <div class="moderation-actions">
                            <button class="join-button"
                                onclick={Callback::from(move |_| toggle.emit((d.clone(), next)))}>
                                { if next { "Make private" } else { "Make public" } }
                            </button>
                            if is_admin {
                                <button class="join-button join-button-danger"
                                    onclick={Callback::from(move |_| delete.emit(del_d.clone()))}>
                                    {"Delete"}
                                </button>
                            }
                        </div>
                    </div>
                }
            }) }
        </>
    }
}

#[function_component]
fn ModeratorsTab() -> Html {
    let reload = use_state(|| 0u32);
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let grants = use_listing::<ModeratorGrant, _, _>(*reload, |api| async move {
        api.list_moderators().await
    });

    let revoke = {
        let (api_ctx, reload) = (api_ctx.clone(), reload.clone());
        Callback::from(move |id: String| {
            let Some(api) = api_of(&api_ctx) else { return };
            let reload = reload.clone();
            wasm_bindgen_futures::spawn_local(async move {
                if let Err(e) = api.revoke_moderator(&id).await {
                    gloo::console::error!(format!("revoke failed: {}", *e));
                }
                reload.set(*reload + 1);
            });
        })
    };

    html! {
        <>
            <p class="moderation-note">
                {"Moderator grants, not people. Revoking a grant removes that moderator's \
                  status and everything they invited."}
            </p>
            { listing(&grants, "No moderator grants.", |g: &ModeratorGrant| {
                let id = g.id.clone();
                let revoke = revoke.clone();
                html! {
                    <div class="moderation-item" key={g.id.clone()}>
                        <p class="moderation-disclosure">
                            { format!("/{}/ — {}", g.board_discriminator, g.label) }
                        </p>
                        <p class="moderation-meta">
                            { if g.redeemed { "Claimed" } else { "Not yet claimed" } }
                        </p>
                        <div class="moderation-actions">
                            <button class="join-button join-button-danger"
                                onclick={Callback::from(move |_| revoke.emit(id.clone()))}>
                                {"Revoke"}
                            </button>
                        </div>
                    </div>
                }
            }) }
        </>
    }
}

#[function_component]
fn LogTab() -> Html {
    let me = use_context::<MeContext>().unwrap_or_default();
    let entries = use_listing::<LogEntry, _, _>(0, |api| async move { api.audit_log().await });

    html! {
        <>
            <p class="moderation-note">
                {
                    if me.me.as_ref().map(|m| m.is_admin()).unwrap_or(false) {
                        "Every moderation action."
                    } else {
                        "Your own moderation actions."
                    }
                }
            </p>
            { listing(&entries, "Nothing recorded yet.", |e: &LogEntry| html! {
                <div class="moderation-item log-item">
                    <p class="moderation-disclosure">
                        { e.action.replace('_', " ") }
                        { e.board_discriminator.as_ref().map(|b| format!(" on /{b}/")).unwrap_or_default() }
                        { e.target.as_ref().map(|t| format!(" ({t})")).unwrap_or_default() }
                    </p>
                    if let Some(detail) = &e.detail {
                        <p class="moderation-meta">{ detail.clone() }</p>
                    }
                    <p class="moderation-meta">
                        { e.created_at.format("%Y-%m-%d %H:%M UTC").to_string() }
                    </p>
                </div>
            }) }
        </>
    }
}
