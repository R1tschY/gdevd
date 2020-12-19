use crate::{Command, Direction, GDeviceModel, RgbColor, Speed};
use ini::ini::Properties;
use ini::Ini;
use std::convert::TryInto;

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
            .unwrap_or_else(|| vec![])
    }

    fn parse_model_config(&self, props: &Properties, model: &dyn GDeviceModel) -> Vec<Command> {
        let model_name = model.get_name();

        match props.get("type") {
            Some("static") => (0..model.get_sectors())
                .map(|i| {
                    Command::ColorSector(
                        self.parse_color_prop(props, model, &format!("color-{}", i)),
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
            )],
            Some("cycle") => vec![Command::Cycle(self.parse_speed(props, model, "speed"))],
            Some("wave") => vec![Command::Wave(
                self.parse_direction(props, model, "direction"),
                self.parse_speed(props, model, "speed"),
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

    fn parse_speed(&self, props: &Properties, model: &dyn GDeviceModel, key: &str) -> Speed {
        if let Some(speed) = props.get(key) {
            if let Ok(speed) = speed.parse::<u16>() {
                return Speed(speed);
            } else {
                warn!(
                    "Invalid speed {} for {}.{} ignored",
                    speed,
                    model.get_name(),
                    key
                );
            }
        }

        Speed(65535 / 2)
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

    pub fn save_command(&mut self, model: &dyn GDeviceModel, cmd: Command) {
        let mut section = self.0.with_section(Some(model.get_name()));

        match cmd {
            Command::ColorSector(color, Some(sector)) => {
                section
                    .set("type", "static")
                    .set(format!("color-{}", sector), color.to_hex());
            }
            Command::ColorSector(color, None) => {
                let mut setter = section.set("type", "static-all");
                for i in 0..model.get_sectors() {
                    setter = setter.set(format!("color-{}", i), color.to_hex());
                }
            }
            Command::Breathe(color, speed) => {
                section
                    .set("type", "breathe")
                    .set("color", color.to_hex())
                    .set("speed", speed.0.to_string());
            }
            Command::Cycle(speed) => {
                section
                    .set("type", "cycle")
                    .set("speed", speed.0.to_string());
            }
            Command::Wave(direction, speed) => {
                section
                    .set("type", "wave")
                    .set(
                        "direction",
                        match direction {
                            Direction::LeftToRight => "left-to-right",
                            Direction::RightToLeft => "right-to-left",
                            Direction::CenterToEdge => "center-to-edge",
                            Direction::EdgeToCenter => "edge-to-center",
                        },
                    )
                    .set("speed", speed.0.to_string());
            }
        }
        self.0.write_to_file(CONFIG_PATH).unwrap_or_else(|err| {
            error!("Failed to write config file {}: {:?}", CONFIG_PATH, err);
        });
    }
}
