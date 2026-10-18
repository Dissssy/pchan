use std::time::Duration;

use gloo::events::EventListener;
use gloo::timers::callback::Interval;
use yew::prelude::*;

const POLL_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Offers a reload when the deployed frontend is newer than the one this page loaded.
///
/// The running version is the hash in the `frontend_bg-<hash>.wasm` preload link that
/// `build.sh` wrote into the `index.html` this page came from. The deployed version comes from
/// `GET /api/v1/version`. Polls on a timer and when the tab regains focus, which is when a
/// long-lived background tab is most likely to be stale.
///
/// It never reloads by itself: a half-written post would be lost.
#[function_component]
pub fn UpdateBanner() -> Html {
    let running = use_memo((), |_| running_version());
    // The deployed version the user dismissed the banner for, so it comes back only if the
    // version changes again.
    let dismissed = use_state(|| None::<String>);
    let latest = use_state(|| None::<String>);

    {
        let latest = latest.clone();
        let enabled = running.is_some();
        use_effect_with(enabled, move |enabled| {
            let mut guards: Option<(Interval, EventListener)> = None;
            if *enabled {
                let check = {
                    let latest = latest.clone();
                    move || {
                        let latest = latest.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Ok(Some(version)) =
                                crate::api::standard_get::<Option<String>>("/api/v1/version").await
                            {
                                latest.set(Some(version));
                            }
                        });
                    }
                };

                check();
                let on_focus = check.clone();
                guards = Some((
                    Interval::new(POLL_INTERVAL.as_millis() as u32, check),
                    EventListener::new(&gloo::utils::window(), "focus", move |_| on_focus()),
                ));
            }
            move || drop(guards)
        });
    }

    let (Some(running), Some(latest)) = ((*running).clone(), (*latest).clone()) else {
        return html! {};
    };
    if running == latest || dismissed.as_deref() == Some(latest.as_str()) {
        return html! {};
    }

    let on_reload = Callback::from(|e: MouseEvent| {
        e.prevent_default();
        let _ = gloo::utils::window().location().reload();
    });
    let on_dismiss = {
        let dismissed = dismissed.clone();
        Callback::from(move |e: MouseEvent| {
            e.prevent_default();
            dismissed.set(Some(latest.clone()));
        })
    };

    html! {
        <div class="update-banner">
            <span>{"A new version of PChan is available. Reloading discards any unsent post."}</span>
            <a class="update-banner-button" onclick={on_reload}>{"Reload"}</a>
            <a class="update-banner-button" onclick={on_dismiss}>{"Dismiss"}</a>
        </div>
    }
}

/// The hash of the wasm bundle this page was loaded with, if it can be found.
fn running_version() -> Option<String> {
    const PREFIX: &str = "frontend_bg-";
    let link = gloo::utils::document()
        .query_selector("link[href*='frontend_bg-']")
        .ok()
        .flatten()?;
    let href = link.get_attribute("href")?;
    let rest = &href[href.find(PREFIX)? + PREFIX.len()..];
    let hash: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
    (!hash.is_empty()).then_some(hash)
}
