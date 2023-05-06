use yew::prelude::*;
use yew_router::prelude::*;

use crate::BaseRoute;

#[function_component]
pub fn MaybeLink(props: &MaybeLinkProps) -> Html {
    if props.link {
        html! {
            <Link<BaseRoute> to={props.to.clone()} >
                {
                    for props.children.iter()
                }
            </Link<BaseRoute>>
        }
    } else {
        html! {
            {
                for props.children.iter()
            }
        }
    }
}

#[derive(Clone, Properties, PartialEq, Debug)]
pub struct MaybeLinkProps {
    pub link: bool,
    pub to: BaseRoute,
    pub children: Children,
}
