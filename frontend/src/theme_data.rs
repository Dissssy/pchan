use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub struct ThemeData {
    pub primary_color: Color,
    pub secondary_color: Color,
    pub topic_color: Color,
    pub bluetext_color: Color,
    pub peetext_color: Color,
    pub border_color: Color,
    pub error_color: Color,
    pub text_color: Color,
    pub secondary_text_color: Color,
    pub border_width: Size,
    pub border_type: BorderType,
    pub link_color: Color,
    pub post_link_valid_color: Color,
    pub post_link_unloaded_color: Color,
    pub post_link_invalid_color: Color,
    pub edge_padding: Size,
    pub animation_speed: Time,
    pub border_radius: Size,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Color {
    Hex(String),
    Name(String),
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Size {
    Em(f64),
    Px(f64),
    Percent(f64),
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum BorderType {
    None,
    Hidden,
    Dotted,
    Dashed,
    Solid,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug)]
pub enum Time {
    Ms(f64),
    S(f64),
}

impl ThemeData {
    pub fn default_dark_theme() -> Self {
        // --primary-color: #282a2e; --secondary-color: #1d1f21; --topic-color: lightblue; --bluetext-color: #a039ff; --peetext-color: #bebe33; --border-color: #242424; --error-color: purple; --text-color: #c5c8c6; --secondary-text-color: #000000; --border-width: 0.05em; --border-type: solid; --link-color: #2c7d31; --post-link-valid-color: #2c7d31; --post-link-unloaded-color: #a039ff; --post-link-invalid-color: #e74c3c; --edge-padding: 2%; --animation-speed: 200ms; --border-radius: 0.4em;
        Self {
            primary_color: Color::Hex("#282a2e".to_string()),
            secondary_color: Color::Hex("#1d1f21".to_string()),
            topic_color: Color::Name("lightblue".to_string()),
            bluetext_color: Color::Hex("#a039ff".to_string()),
            peetext_color: Color::Hex("#bebe33".to_string()),
            border_color: Color::Hex("#242424".to_string()),
            error_color: Color::Name("purple".to_string()),
            text_color: Color::Hex("#c5c8c6".to_string()),
            secondary_text_color: Color::Hex("#000000".to_string()),
            border_width: Size::Em(0.05),
            border_type: BorderType::Solid,
            link_color: Color::Hex("#2c7d31".to_string()),
            post_link_valid_color: Color::Hex("#2c7d31".to_string()),
            post_link_unloaded_color: Color::Hex("#a039ff".to_string()),
            post_link_invalid_color: Color::Hex("#e74c3c".to_string()),
            edge_padding: Size::Percent(2.0),
            animation_speed: Time::Ms(200.0),
            border_radius: Size::Em(0.4),
        }
    }

    pub fn default_light_theme() -> Self {
        // --primary-color: #f0e0d6; --secondary-color: #ffe; --topic-color: purple; --bluetext-color: #a039ff; --peetext-color: #bebe33; --border-color: #d9bfb7; --error-color: purple; --text-color: maroon; --secondary-text-color: #000000; --border-width: 0.05em; --border-type: solid; --link-color: blue; --post-link-valid-color: #2c7d31; --post-link-unloaded-color: #a039ff; --post-link-invalid-color: #e74c3c; --edge-padding: 2%; --animation-speed: 200ms; --border-radius: 0.4em;
        Self {
            primary_color: Color::Hex("#f0e0d6".to_string()),
            secondary_color: Color::Hex("#ffffee".to_string()),
            topic_color: Color::Name("purple".to_string()),
            bluetext_color: Color::Hex("#a039ff".to_string()),
            peetext_color: Color::Hex("#bebe33".to_string()),
            border_color: Color::Hex("#d9bfb7".to_string()),
            error_color: Color::Name("purple".to_string()),
            text_color: Color::Name("maroon".to_string()),
            secondary_text_color: Color::Hex("#000000".to_string()),
            border_width: Size::Em(0.05),
            border_type: BorderType::Solid,
            link_color: Color::Name("blue".to_string()),
            post_link_valid_color: Color::Hex("#2c7d31".to_string()),
            post_link_unloaded_color: Color::Hex("#a039ff".to_string()),
            post_link_invalid_color: Color::Hex("#e74c3c".to_string()),
            edge_padding: Size::Percent(2.0),
            animation_speed: Time::Ms(200.0),
            border_radius: Size::Em(0.4),
        }
    }

    pub fn to_css_str(&self) -> String {
        format!(
            "--primary-color: {}; --secondary-color: {}; --topic-color: {}; --bluetext-color: {}; --peetext-color: {}; --border-color: {}; --error-color: {}; --text-color: {}; --secondary-text-color: {}; --border-width: {}; --border-type: {}; --link-color: {}; --post-link-valid-color: {}; --post-link-unloaded-color: {}; --post-link-invalid-color: {}; --edge-padding: {}; --animation-speed: {}; --border-radius: {};",
            self.primary_color.to_css_str(),
            self.secondary_color.to_css_str(),
            self.topic_color.to_css_str(),
            self.bluetext_color.to_css_str(),
            self.peetext_color.to_css_str(),
            self.border_color.to_css_str(),
            self.error_color.to_css_str(),
            self.text_color.to_css_str(),
            self.secondary_text_color.to_css_str(),
            self.border_width.to_css_str(),
            self.border_type.to_css_str(),
            self.link_color.to_css_str(),
            self.post_link_valid_color.to_css_str(),
            self.post_link_unloaded_color.to_css_str(),
            self.post_link_invalid_color.to_css_str(),
            self.edge_padding.to_css_str(),
            self.animation_speed.to_css_str(),
            self.border_radius.to_css_str(),
        )
    }

    pub fn set_color(&mut self, field: String, value: Color) -> Result<(), String> {
        match field.as_str() {
            "primary_color" => self.primary_color = value,
            "secondary_color" => self.secondary_color = value,
            "topic_color" => self.topic_color = value,
            "bluetext_color" => self.bluetext_color = value,
            "peetext_color" => self.peetext_color = value,
            "border_color" => self.border_color = value,
            "error_color" => self.error_color = value,
            "text_color" => self.text_color = value,
            "secondary_text_color" => self.secondary_text_color = value,
            "link_color" => self.link_color = value,
            "post_link_valid_color" => self.post_link_valid_color = value,
            "post_link_unloaded_color" => self.post_link_unloaded_color = value,
            "post_link_invalid_color" => self.post_link_invalid_color = value,
            _ => return Err(format!("Invalid field: {}", field)),
        };
        Ok(())
    }

    pub fn get_color(&self, field: String) -> Result<Color, String> {
        match field.as_str() {
            "primary_color" => Ok(self.primary_color.clone()),
            "secondary_color" => Ok(self.secondary_color.clone()),
            "topic_color" => Ok(self.topic_color.clone()),
            "bluetext_color" => Ok(self.bluetext_color.clone()),
            "peetext_color" => Ok(self.peetext_color.clone()),
            "border_color" => Ok(self.border_color.clone()),
            "error_color" => Ok(self.error_color.clone()),
            "text_color" => Ok(self.text_color.clone()),
            "secondary_text_color" => Ok(self.secondary_text_color.clone()),
            "link_color" => Ok(self.link_color.clone()),
            "post_link_valid_color" => Ok(self.post_link_valid_color.clone()),
            "post_link_unloaded_color" => Ok(self.post_link_unloaded_color.clone()),
            "post_link_invalid_color" => Ok(self.post_link_invalid_color.clone()),
            _ => Err(format!("Invalid field: {}", field)),
        }
    }

    pub fn get_size(&self, field: String) -> Result<Size, String> {
        match field.as_str() {
            "border_width" => Ok(self.border_width.clone()),
            "edge_padding" => Ok(self.edge_padding.clone()),
            "border_radius" => Ok(self.border_radius.clone()),
            _ => Err(format!("Invalid field: {}", field)),
        }
    }

    pub fn set_size(&mut self, field: String, value: Size) -> Result<(), String> {
        match field.as_str() {
            "border_width" => self.border_width = value,
            "edge_padding" => self.edge_padding = value,
            "border_radius" => self.border_radius = value,
            _ => return Err(format!("Invalid field: {}", field)),
        };
        Ok(())
    }
}

impl Color {
    pub fn to_css_str(&self) -> String {
        match self {
            Color::Hex(s) => s.clone(),
            Color::Name(s) => s.clone(),
        }
    }
}

impl Size {
    pub fn to_css_str(&self) -> String {
        match self {
            Size::Em(f) => format!("{}em", f),
            Size::Px(f) => format!("{}px", f),
            Size::Percent(f) => format!("{}%", f),
        }
    }
}

impl BorderType {
    pub fn to_css_str(&self) -> String {
        match self {
            BorderType::None => "none".to_string(),
            BorderType::Hidden => "hidden".to_string(),
            BorderType::Dotted => "dotted".to_string(),
            BorderType::Dashed => "dashed".to_string(),
            BorderType::Solid => "solid".to_string(),
            BorderType::Double => "double".to_string(),
            BorderType::Groove => "groove".to_string(),
            BorderType::Ridge => "ridge".to_string(),
            BorderType::Inset => "inset".to_string(),
            BorderType::Outset => "outset".to_string(),
        }
    }
}

impl Time {
    pub fn to_css_str(&self) -> String {
        match self {
            Time::Ms(f) => format!("{}ms", f),
            Time::S(f) => format!("{}s", f),
        }
    }
}
