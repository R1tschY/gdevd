use std::convert::TryInto;

use ini::{Ini, Properties, SectionSetter};

use crate::{Brightness, Command, Direction, GDeviceModel, RgbColor, Speed};

const CONFIG_PATH: &str = "/etc/gdevd.conf";

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
            .unwrap_or_default()
    }

    fn parse_model_config(&self, props: &Properties, model: &dyn GDeviceModel) -> Vec<Command> {
        let model_name = model.get_name();

        match props.get("type") {
            Some("static") => (0..model.get_sectors())
                .map(|i| {
                    Command::ColorSector(
                        self.parse_color_prop(props, model, &format!("color-{i}")),
                        Some(i),
                    )
                })
                .collect(),
            Some("static-all") => vec![Command::ColorSector(
                self.parse_color_prop(props, model, "color-0"),
                None,
            )],
            Some("breath") => vec![Command::Breathe(
                self.parse_color_prop(props, model, "color"),
                self.parse_speed(props, model, "speed"),
                self.parse_brightness(props, model, "brightness"),
            )],
            Some("cycle") => vec![Command::Cycle(
                self.parse_speed(props, model, "speed"),
                self.parse_brightness(props, model, "brightness"),
            )],
            Some("wave") => vec![Command::Wave(
                self.parse_direction(props, model, "direction"),
                self.parse_speed(props, model, "speed"),
                self.parse_brightness(props, model, "brightness"),
            )],
            Some("startEffect") => vec![Command::StartEffect(
                self.parse_bool(props, model, "state").unwrap_or(true),
            )],
            Some(unknown) => {
                warn!("Unknown color mode `{}` for {}", unknown, model_name);
                vec![]
            }
            None => vec![],
        }
    }

    fn parse_color_prop(
        &self,
        props: &Properties,
        model: &dyn GDeviceModel,
        key: &str,
    ) -> RgbColor {
        if let Some(color) = props.get(key) {
            if let Ok(rgb) = RgbColor::from_hex(color) {
                return rgb;
            } else {
                warn!(
                    "Invalid RGB hex color {} for {}.{} ignored",
                    color,
                    model.get_name(),
                    key
                );
            }
        }

        model.get_default_color()
    }

    fn parse_speed(
        &self,
        props: &Properties,
        model: &dyn GDeviceModel,
        key: &str,
    ) -> Option<Speed> {
        if let Some(speed) = props.get(key) {
            if let Ok(speed) = speed.parse::<u16>() {
                return Some(Speed(speed));
            } else {
                warn!(
                    "Invalid speed {} for {}.{} ignored",
                    speed,
                    model.get_name(),
                    key
                );
            }
        }

        None
    }

    fn parse_brightness(
        &self,
        props: &Properties,
        model: &dyn GDeviceModel,
        key: &str,
    ) -> Option<Brightness> {
        if let Some(brightness) = props.get(key) {
            if let Ok(brightness) = brightness.parse::<u8>() {
                if brightness <= 100 {
                    return Some(Brightness(brightness));
                }
            }
            warn!(
                "Invalid brightness {} for {}.{} ignored",
                brightness,
                model.get_name(),
                key
            );
        }

        None
    }

    fn parse_direction(
        &self,
        props: &Properties,
        model: &dyn GDeviceModel,
        key: &str,
    ) -> Direction {
        if let Some(direction) = props.get(key) {
            direction.try_into().unwrap_or_else(|_err| {
                warn!(
                    "Invalid direction {} for {}.{} ignored",
                    direction,
                    model.get_name(),
                    key
                );
                Direction::LeftToRight
            })
        } else {
            Direction::LeftToRight
        }
    }

    fn parse_bool(&self, props: &Properties, model: &dyn GDeviceModel, key: &str) -> Option<bool> {
        if let Some(boolean) = props.get(key) {
            if let Ok(boolean) = boolean.parse::<bool>() {
                return Some(boolean);
            } else {
                warn!(
                    "Invalid speed {} for {}.{} ignored",
                    boolean,
                    model.get_name(),
                    key
                );
            }
        }

        None
    }

    pub fn save_command(&mut self, model: &dyn GDeviceModel, cmd: Command) {
        let mut section = self.0.with_section(Some(model.get_name()));

        match cmd {
            Command::ColorSector(color, Some(sector)) => {
                section
                    .set("type", "static")
                    .set(format!("color-{sector}"), color.to_hex());
            }
            Command::ColorSector(color, None) => {
                let mut setter = section.set("type", "static-all");
                for i in 0..model.get_sectors() {
                    setter = setter.set(format!("color-{i}"), color.to_hex());
                }
            }
            Command::Breathe(color, speed, brightness) => {
                let section = section.set("type", "breathe").set("color", color.to_hex());
                let section = Self::set_speed(section, speed);
                Self::set_brightness(section, brightness);
            }
            Command::Cycle(speed, brightness) => {
                let section = section.set("type", "cycle");
                let section = Self::set_speed(section, speed);
                Self::set_brightness(section, brightness);
            }
            Command::Wave(direction, speed, brightness) => {
                let section = section.set("type", "wave").set(
                    "direction",
                    match direction {
                        Direction::LeftToRight => "left-to-right",
                        Direction::RightToLeft => "right-to-left",
                        Direction::CenterToEdge => "center-to-edge",
                        Direction::EdgeToCenter => "edge-to-center",
                    },
                );
                let section = Self::set_speed(section, speed);
                Self::set_brightness(section, brightness);
            }
            Command::StartEffect(state) => {
                section
                    .set("type", "startEffect")
                    .set("state", if state { "true" } else { "false" });
            }
            Command::Blend(speed, brightness) => {
                let section = section.set("type", "blend");
                let section = Self::set_speed(section, speed);
                Self::set_brightness(section, brightness);
            }
            Command::Dpi(dpi) => {
                section.set("type", "dpi").set("dpi", dpi.0.to_string());
            }
        }
        self.0.write_to_file(CONFIG_PATH).unwrap_or_else(|err| {
            error!("Failed to write config file {}: {:?}", CONFIG_PATH, err);
        });
    }

    fn set_speed<'a>(
        section: &'a mut SectionSetter<'a>,
        speed: Option<Speed>,
    ) -> &'a mut SectionSetter<'a> {
        if let Some(speed) = speed {
            section.set("speed", speed.0.to_string())
        } else {
            section.delete(&"speed")
        }
    }

    fn set_brightness<'a>(
        section: &'a mut SectionSetter<'a>,
        brightness: Option<Brightness>,
    ) -> &'a mut SectionSetter<'a> {
        if let Some(brightness) = brightness {
            section.set("brightness", brightness.0.to_string())
        } else {
            section.delete(&"brightness")
        }
    }
}
