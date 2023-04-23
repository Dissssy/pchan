use yew::prelude::*;

#[function_component]
pub fn Banner(props: &Props) -> Html {
    let banner = use_state(|| None);

    {
        let banner = banner.clone();
        let props = props.clone();
        use_effect_with_deps(
            move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    crate::API
                        .lock()
                        .await
                        .get_banner(props.board_discriminator.clone(), banner)
                        .await;
                });
                || {}
            },
            (),
        );
    }

    match *banner {
        Some(ref b) => {
            html! {
                <div class="banner">
                    {
                        match b.href {
                            Some(ref link) => {
                                html! {
                                    <a href={link.clone()}>
                                        <img class="banner-image" src={b.path.clone()} />
                                    </a>
                                }
                            }
                            None => {
                                html! {
                                    <img class="banner-image" src={b.path.clone()} />
                                }
                            }
                        }
                    }
                </div>
            }
        }
        None => {
            html! {
                <></>
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug, Properties)]
pub struct Props {
    pub board_discriminator: String,
}
