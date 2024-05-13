use yew::prelude::*;
use yew_router::prelude::*;

use crate::{api::ApiState, ApiContext, BaseRoute};

#[function_component]
pub fn BannerAd() -> Html {
    let route = use_route::<BaseRoute>();
    let api_ctx = use_context::<Option<ApiContext>>().flatten();
    let banner = use_state(|| ApiState::Pending);

    {
        let banner = banner.clone();
        let api_ctx = api_ctx;
        use_effect_with(route, move |route| {
            if let ApiState::Loaded(_) = *banner {
            } else {
                banner.set(ApiState::Loading);
            }
            let route = route.clone();
            let api_ctx = api_ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                match api_ctx {
                    Some(api_ctx) => match api_ctx.api {
                        Err(e) => {
                            banner.set(ApiState::Error(e));
                        }
                        Ok(api) => {
                            if let Some(route) = route.and_then(|r| r.board_discriminator()) {
                                match api.get_banner(&route).await {
                                    Err(e) => {
                                        banner.set(ApiState::Error(e));
                                    }
                                    Ok(thisbanner) => {
                                        banner.set(ApiState::Loaded(thisbanner));
                                    }
                                };
                            } else {
                                banner.set(ApiState::ContextError(AttrValue::from("BoardContext")));
                            }
                        }
                    },
                    _ => {
                        banner.set(ApiState::ContextError(AttrValue::from("ApiContext")));
                    }
                }
            });
        });
    }

    match banner.standard_html("BannerAd", |banner| {
        html! {
            <div class="randomized-banner-image">
                {
                    if let Some(url) = &banner.href {
                        html! {
                            <a href={url.clone()}>
                                <img src={banner.path.clone()} />
                            </a>
                        }
                    } else {
                        html! {
                            <img src={banner.path.clone()} />
                        }
                    }
                }
            </div>
        }
    }) {
        Ok(html) => html,
        Err(e) => {
            gloo::console::error!(format!("Error rendering BannerAd: {}", *e));
            html! {}
        }
    }
}
