//! Acrobat's Specify Settings uses labelled fields, while portable actions keep their stable
//! argument format. Split only the setting separators so pipes and newlines in mark text survive.

#[derive(Clone, Debug, PartialEq)]
pub struct MarkSettings {
    pub text: String,
    pub size: String,
    pub opacity: String,
    pub rotation: String,
    pub x: String,
    pub y: String,
    pub font: String,
    pub pages: String,
    pub position: i32,
}

impl Default for MarkSettings {
    fn default() -> Self {
        Self { text: String::new(), size: "36".into(), opacity: "0.5".into(), rotation: "45".into(),
            x: "0".into(), y: "0".into(), font: "Helvetica".into(), pages: "all".into(), position: 4 }
    }
}

impl MarkSettings {
    pub fn decode(tool: &str, argument: &str) -> Self {
        let mut settings = Self::default();
        if tool == "watermark_options" {
            let fields: Vec<_> = argument.splitn(8, '|').collect();
            if fields.len() == 8 {
                settings.size = fields[0].into(); settings.opacity = fields[1].into();
                settings.rotation = fields[2].into(); settings.x = fields[3].into(); settings.y = fields[4].into();
                settings.font = fields[5].into(); settings.pages = fields[6].into(); settings.text = fields[7].into();
            }
        } else if tool == "header_footer_options" {
            settings.size = "8".into();
            let fields: Vec<_> = argument.splitn(5, '|').collect();
            if fields.len() == 5 {
                settings.position = fields[0].parse().unwrap_or(4); settings.size = fields[1].into();
                settings.font = fields[2].into(); settings.pages = fields[3].into(); settings.text = fields[4].into();
            }
        }
        settings
    }

    pub fn encode(&self, tool: &str) -> String {
        if tool == "header_footer_options" {
            format!("{}|{}|{}|{}|{}", self.position, self.size, self.font, self.pages, self.text)
        } else {
            format!("{}|{}|{}|{}|{}|{}|{}|{}", self.size, self.opacity, self.rotation, self.x, self.y, self.font, self.pages, self.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_core::pdfcraft::engine::{actions::{Step, command_edit}, Edit};

    #[test]
    fn labelled_mark_settings_round_trip_and_drive_the_selected_pages_and_appearance() {
        let watermark = MarkSettings { text: "Review | final\nConfidential".into(), size: "24".into(), opacity: "0.35".into(),
            rotation: "-30".into(), x: "12".into(), y: "-18".into(), font: "Courier".into(), pages: "2-3".into(), ..Default::default() };
        for tool in ["watermark_options", "header_footer_options"] {
            let mut settings = watermark.clone(); settings.position = 2;
            let argument = settings.encode(tool);
            let decoded = MarkSettings::decode(tool, &argument);
            assert_eq!(decoded.encode(tool), argument);
            let edit = command_edit(&Step::from_id(tool, &argument).unwrap(), 4).unwrap();
            match edit {
                Edit::AddWatermark { pages, settings: mark, .. } => {
                    assert_eq!(pages, [1, 2]); assert_eq!(mark.text, settings.text);
                    assert_eq!(mark.font_size, 24.0); assert_eq!(mark.opacity, 0.35);
                    assert_eq!(mark.rotation, -30.0); assert_eq!(mark.offset, [12.0, -18.0]);
                }
                Edit::AddHeaderFooter { pages, settings: mark, .. } => {
                    assert_eq!(pages, [1, 2]); assert_eq!(mark.text[2], settings.text);
                    assert_eq!(mark.font_size, 24.0);
                }
                _ => panic!("Unexpected mark edit"),
            }
            assert!(command_edit(&Step::from_id(tool, &argument).unwrap(), 1).is_err());
            let mut invalid = settings; invalid.size = "NaN".into();
            assert!(Step::from_id(tool, &invalid.encode(tool)).is_none());
        }
    }
}
