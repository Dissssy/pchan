#![allow(irrefutable_let_patterns)]
// simple yew component that displays "Powered by Pchan Frontend v{frontend version} & Backend v{backend version}" and a link to the github repository

use yew::prelude::*;

macro_rules! frontend_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

macro_rules! backend_version {
    () => {
        // extract the backend version from the Cargo.toml file located in the workspace/backend directory
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../backend/Cargo.toml"))
            .lines()
            .find(|line| line.starts_with("version"))
            .expect("Could not find version in Cargo.toml")
            .split('=')
            .nth(1)
            .expect("Could not find version in Cargo.toml")
            .trim()
            .replace("\"", "")
    };
}


macro_rules! string_hash_to_colorcode {
    ($s:expr) => {
        {
            let mut hash = 0u32;
            for c in $s.chars() {
                hash = hash.wrapping_mul(31).wrapping_add(c as u32);
            }
            let color = hash & 0xFFFFFF;
            let r = (color >> 16) & 0xFF;
            let g = (color >> 8) & 0xFF;
            let b = color & 0xFF;
            // maximize saturation
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            let diff = max - min;
            let mid = (max + min) / 2;
            let mut hue = 0;
            if diff != 0 {
                if max == r {
                    hue = ((g - b) * 60 / diff + 360) % 360;
                } else if max == g {
                    hue = ((b - r) * 60 / diff + 120) % 360;
                } else {
                    hue = ((r - g) * 60 / diff + 240) % 360;
                }
            }
            let saturation = if max == 0 { 0 } else { diff * 255 / max };
            let lightness = mid * 255 / max;
            let color = (hue << 16) | (saturation << 8) | lightness;
            format!("#{:06X}", color & 0xFFFFFF)
        }
    };
}

#[function_component]
pub fn PoweredBy() -> Html {
    html!{
        <div class="powered-by">
            <a href="https://github.com/Dissssy/pchan">
                <span>{"Powered by Pchan, Frontend "}</span>
                <span style={format!("color: {};", string_hash_to_colorcode!(
                    &format!("frontend {}", frontend_version!())
                ))} class="powered-by-frontend-version">
                    {format!("v{}", frontend_version!())}
                </span>
                <span>{", Backend "}</span>
                <span style={format!("color: {};", string_hash_to_colorcode!(
                    &format!("backend {}", backend_version!())
                ))} class="powered-by-backend-version">
                    {format!("v{}", backend_version!())}
                </span>
            </a>
        </div>
    }
}

// fn string_hash_to_colorcode(s: &str) -> String {
//     let mut hash = 0u32;
//     for c in s.chars() {
//         hash = hash.wrapping_mul(31).wrapping_add(c as u32);
//     }
//     let color = hash & 0xFFFFFF;
//     let r = (color >> 16) & 0xFF;
//     let g = (color >> 8) & 0xFF;
//     let b = color & 0xFF;
//     // maximize saturation
//     let max = r.max(g).max(b);
//     let min = r.min(g).min(b);
//     let diff = max - min;
//     let mid = (max + min) / 2;
//     let mut hue = 0;
//     if diff != 0 {
//         if max == r {
//             hue = ((g - b) * 60 / diff + 360) % 360;
//         } else if max == g {
//             hue = ((b - r) * 60 / diff + 120) % 360;
//         } else {
//             hue = ((r - g) * 60 / diff + 240) % 360;
//         }
//     }
//     let saturation = if max == 0 { 0 } else { diff * 255 / max };
//     let lightness = mid * 255 / max;
//     let color = (hue << 16) | (saturation << 8) | lightness;
//     format!("#{:06X}", color & 0xFFFFFF)
// }
