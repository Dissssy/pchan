use yew::prelude::*;
use yew_hooks::use_local_storage;

/// Key read by the reporter in `index.html`; it sends unless this is the string "false".
const ERROR_REPORTS_KEY: &str = "error_reports";

#[function_component]
pub fn ErrorReportEditor() -> Html {
    let stored = use_local_storage::<bool>(ERROR_REPORTS_KEY.to_owned());
    let enabled = (*stored).unwrap_or(true);
    let onchange = {
        let stored = stored.clone();
        Callback::from(move |_: Event| stored.set(!enabled))
    };

    html! {
        <div class="error-report-editor">
            <label>
                <input type="checkbox" checked={enabled} {onchange} />
                {" Send anonymous error reports"}
            </label>
            <p class="error-report-note">
                {"When the site crashes, send the error message and where it happened, which \
                  kind of page you were on (not which board or thread), your browser name and \
                  version, and the site version. Nothing about you or what you have written, \
                  and identical errors are only counted."}
            </p>
        </div>
    }
}
