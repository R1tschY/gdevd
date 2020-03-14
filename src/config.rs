use crate::{Command, GDeviceModel, RgbColor};
use ini::ini::{ParseError, Properties};
use ini::Ini;

const CONFIG_PATH: &str = "/etc/g213d.conf";

pub struct Config(Ini);

impl Config {
    pub fn load() -> Self {
        let ini = Ini::load_from_file(CONFIG_PATH).unwrap_or_else(|err| {
            warn!(
                "Config file {} has invalid format and is ignored: {:?}",
                CONFIG_PATH, err
            );
            Ini::new()
        });

        Self(ini)
    }

    pub fn commands_for(&self, model: &dyn GDeviceModel) -> Vec<Command> {
        let model_name = model.get_name();
        self.0
            .section(Some(model_name))
            .map(|props| self.parse_model_config(props, model))
            .unwrap_or(vec![])
    }

    fn parse_model_config(&self, props: &Properties, model: &dyn GDeviceModel) -> Vec<Command> {
        let model_name = model.get_name();
        let default_color = model.get_default_color();

        match props.get("type") {
            Some("static") => (0..=model.get_sectors())
                .map(|i| {
                    self.parse_color_prop(props, model_name, &format!("color-{}", i))
                        .map(|rgb| Command::ColorSector(rgb, Some(i)))
                        .unwrap_or_else(|| Command::ColorSector(default_color, Some(i)))
                })
                .collect(),
            Some("static-all") => self
                .parse_color_prop(props, model_name, "color-0")
                .map(|rgb| Command::ColorSector(rgb, None))
                .into_iter()
                .collect(),
            _ => unimplemented!(),
        }
    }

    fn parse_color_prop(&self, props: &Properties, model: &str, key: &str) -> Option<RgbColor> {
        if let Some(color) = props.get(key) {
            if let Ok(rgb) = RgbColor::from_hex(color) {
                Some(rgb)
            } else {
                warn!(
                    "Invalid RGB hex color {} for {}.{} ignored",
                    color, model, key
                );
                None
            }
        } else {
            None
        }
    }

    pub fn save_command(&mut self, model: &dyn GDeviceModel, cmd: &Command) {
        let mut section = self.0.with_section(Some(model.get_name()));

        match cmd {
            Command::ColorSector(color, Some(sector)) => {
                section
                    .set("type", "static")
                    .set(format!("color-{}", sector), color.to_hex());
            }
            Command::ColorSector(color, None) => {
                let mut setter = section.set("type", "static-all");
                for i in 0..=model.get_sectors() {
                    setter = setter.set(format!("color-{}", i), color.to_hex());
                }
            }
            _ => unimplemented!(),
        }
        self.0.write_to_file(CONFIG_PATH).unwrap_or_else(|err| {
            error!("Failed to write config file {}: {:?}", CONFIG_PATH, err);
        });
    }
}
