//! Strict, pure text generation for a few Minecraft Java command templates.
//! Returned strings are suggestions only and are never executed here.

#[cfg(test)]
use serde_json::Value;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaRelease {
    V1_20_1,
    V1_21_1,
    V1_21_4,
}

impl JavaRelease {
    pub fn parse(value: &str) -> Result<Self, BuildError> {
        match value {
            "1.20.1" => Ok(Self::V1_20_1),
            "1.21.1" => Ok(Self::V1_21_1),
            "1.21.4" => Ok(Self::V1_21_4),
            _ => Err(BuildError::UnsupportedRelease),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V1_20_1 => "1.20.1",
            Self::V1_21_1 => "1.21.1",
            Self::V1_21_4 => "1.21.4",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Target {
    SelfPlayer,
    NearestPlayer,
    AllPlayers,
    PlayerName(String),
}

impl Target {
    fn render(&self) -> Result<&str, BuildError> {
        match self {
            Self::SelfPlayer => Ok("@s"),
            Self::NearestPlayer => Ok("@p"),
            Self::AllPlayers => Ok("@a"),
            Self::PlayerName(name) if valid_player_name(name) => Ok(name),
            Self::PlayerName(_) => Err(BuildError::InvalidTarget),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Coordinate {
    Absolute(f64),
    Relative(f64),
}

impl Coordinate {
    fn render(self) -> Result<String, BuildError> {
        let (relative, value) = match self {
            Self::Absolute(value) => (false, value),
            Self::Relative(value) => (true, value),
        };
        if !value.is_finite() || value.abs() > 30_000_000.0 {
            return Err(BuildError::InvalidCoordinate);
        }
        if relative && value == 0.0 {
            return Ok("~".to_owned());
        }
        let value = if value == 0.0 {
            "0".to_owned()
        } else {
            value.to_string()
        };
        Ok(if relative { format!("~{value}") } else { value })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CommandTemplate {
    Teleport {
        target: Target,
        x: Coordinate,
        y: Coordinate,
        z: Coordinate,
    },
    Give {
        target: Target,
        item_id: String,
        quantity: u8,
    },
    Effect {
        target: Target,
        effect_id: String,
        seconds: u32,
        amplifier: u8,
        hide_particles: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildError {
    UnsupportedRelease,
    InvalidTarget,
    InvalidCoordinate,
    InvalidResourceId,
    InvalidQuantity,
    InvalidDuration,
    InvalidAmplifier,
}

impl fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedRelease => "unsupported Java release",
            Self::InvalidTarget => "target must be a supported player selector or player name",
            Self::InvalidCoordinate => "coordinate must be finite and within ±30000000",
            Self::InvalidResourceId => "resource ID must be a simple namespaced identifier",
            Self::InvalidQuantity => "quantity must be from 1 through 64",
            Self::InvalidDuration => "duration must be from 1 through 1000000 seconds",
            Self::InvalidAmplifier => "amplifier must be from 0 through 127",
        })
    }
}

impl std::error::Error for BuildError {}

/// Builds plain command text for the crate's explicit templates.
/// The release is an allowlist; it does not mean the resulting command was
/// tested against or executed by a Minecraft server.
pub fn build_command(
    release: JavaRelease,
    template: &CommandTemplate,
) -> Result<String, BuildError> {
    let _release = release;
    match template {
        CommandTemplate::Teleport { target, x, y, z } => Ok(format!(
            "/tp {} {} {} {}",
            target.render()?,
            x.render()?,
            y.render()?,
            z.render()?
        )),
        CommandTemplate::Give {
            target,
            item_id,
            quantity,
        } => {
            if !(1..=64).contains(quantity) {
                return Err(BuildError::InvalidQuantity);
            }
            if !valid_resource_id(item_id) {
                return Err(BuildError::InvalidResourceId);
            }
            Ok(format!("/give {} {item_id} {quantity}", target.render()?))
        }
        CommandTemplate::Effect {
            target,
            effect_id,
            seconds,
            amplifier,
            hide_particles,
        } => {
            if !(1..=1_000_000).contains(seconds) {
                return Err(BuildError::InvalidDuration);
            }
            if *amplifier > 127 {
                return Err(BuildError::InvalidAmplifier);
            }
            if !valid_resource_id(effect_id) {
                return Err(BuildError::InvalidResourceId);
            }
            Ok(format!(
                "/effect give {} {effect_id} {seconds} {amplifier} {hide_particles}",
                target.render()?
            ))
        }
    }
}

fn valid_player_name(value: &str) -> bool {
    (1..=16).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn valid_resource_id(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else {
        return false;
    };
    !namespace.is_empty()
        && !path.is_empty()
        && namespace.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
        && path.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'_' | b'.' | b'-' | b'/')
        })
}

#[cfg(test)]
fn parse_target(value: &str) -> Result<Target, BuildError> {
    match value {
        "@s" => Ok(Target::SelfPlayer),
        "@p" => Ok(Target::NearestPlayer),
        "@a" => Ok(Target::AllPlayers),
        _ if valid_player_name(value) => Ok(Target::PlayerName(value.into())),
        _ => Err(BuildError::InvalidTarget),
    }
}

#[cfg(test)]
fn parse_coordinate(value: &str) -> Result<Coordinate, BuildError> {
    let (relative, number) = value
        .strip_prefix('~')
        .map_or((false, value), |rest| (true, rest));
    if relative && number.is_empty() {
        return Ok(Coordinate::Relative(0.0));
    }
    let bytes = number.as_bytes();
    let start = usize::from(bytes.first() == Some(&b'-'));
    if start == bytes.len()
        || bytes[start..]
            .iter()
            .any(|byte| !byte.is_ascii_digit() && *byte != b'.')
    {
        return Err(BuildError::InvalidCoordinate);
    }
    let mut dots = 0;
    let mut digits = 0;
    for byte in &bytes[start..] {
        if *byte == b'.' {
            dots += 1;
        } else {
            digits += 1;
        }
    }
    if dots > 1 || digits == 0 || (dots == 1 && bytes.last() == Some(&b'.')) {
        return Err(BuildError::InvalidCoordinate);
    }
    let amount = number
        .parse::<f64>()
        .map_err(|_| BuildError::InvalidCoordinate)?;
    Ok(if relative {
        Coordinate::Relative(amount)
    } else {
        Coordinate::Absolute(amount)
    })
}

#[cfg(test)]
fn input_error(locale: &str, code: &str, english: &str, chinese: &str) -> tool_form::Failure {
    tool_form::Failure {
        code: code.into(),
        message: localized(locale, english, chinese).into(),
    }
}

#[cfg(test)]
fn localized<'a>(locale: &str, english: &'a str, chinese: &'a str) -> &'a str {
    if locale == "zh-CN" { chinese } else { english }
}

#[cfg(test)]
fn build_input(locale: &str, input: &Value) -> Result<tool_form::Output, tool_form::Failure> {
    if !matches!(locale, "en" | "zh-CN") {
        return Err(input_error(
            "en",
            "UNSUPPORTED_LOCALE",
            "This language is not supported.",
            "不支持此语言。",
        ));
    }
    let invalid = || {
        input_error(
            locale,
            "INVALID_REQUEST",
            "The command form is invalid.",
            "指令表单无效。",
        )
    };
    let fields = input.as_object().ok_or_else(invalid)?;
    const FIELD_NAMES: [&str; 11] = [
        "version",
        "template",
        "target",
        "x",
        "y",
        "z",
        "item_id",
        "quantity",
        "effect_id",
        "seconds",
        "amplifier",
    ];
    if fields.len() != FIELD_NAMES.len() + 1
        || FIELD_NAMES.iter().any(|field| !fields.contains_key(*field))
        || !fields.contains_key("hide_particles")
    {
        return Err(invalid());
    }
    let string = |key: &str| fields.get(key).and_then(Value::as_str).ok_or_else(invalid);
    let release = JavaRelease::parse(string("version")?).map_err(|_| {
        input_error(
            locale,
            "UNSUPPORTED_RELEASE",
            "Choose one of the supported Java releases.",
            "请选择受支持的 Java 版本。",
        )
    })?;
    let target = parse_target(string("target")?).map_err(|error| build_error(locale, error))?;
    let template = match string("template")? {
        "teleport" => CommandTemplate::Teleport {
            target,
            x: parse_coordinate(string("x")?).map_err(|error| build_error(locale, error))?,
            y: parse_coordinate(string("y")?).map_err(|error| build_error(locale, error))?,
            z: parse_coordinate(string("z")?).map_err(|error| build_error(locale, error))?,
        },
        "give" => CommandTemplate::Give {
            target,
            item_id: string("item_id")?.into(),
            quantity: u8::try_from(
                fields
                    .get("quantity")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| build_error(locale, BuildError::InvalidQuantity))?,
        },
        "effect" => CommandTemplate::Effect {
            target,
            effect_id: string("effect_id")?.into(),
            seconds: u32::try_from(
                fields
                    .get("seconds")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| build_error(locale, BuildError::InvalidDuration))?,
            amplifier: u8::try_from(
                fields
                    .get("amplifier")
                    .and_then(Value::as_u64)
                    .ok_or_else(invalid)?,
            )
            .map_err(|_| build_error(locale, BuildError::InvalidAmplifier))?,
            hide_particles: fields
                .get("hide_particles")
                .and_then(Value::as_bool)
                .ok_or_else(invalid)?,
        },
        _ => return Err(invalid()),
    };
    let command = build_command(release, &template).map_err(|error| build_error(locale, error))?;
    Ok(tool_form::Output {
        title: localized(locale, "Generated command", "生成的指令").into(),
        blocks: vec![tool_form::TextBlock {
            label: Some(localized(locale, "Command", "指令").into()),
            text: command,
        }],
    })
}

#[cfg(test)]
fn build_error(locale: &str, error: BuildError) -> tool_form::Failure {
    let (code, english, chinese) = match error {
        BuildError::UnsupportedRelease => (
            "UNSUPPORTED_RELEASE",
            "Choose one of the supported Java releases.",
            "请选择受支持的 Java 版本。",
        ),
        BuildError::InvalidTarget => (
            "INVALID_TARGET",
            "Use @s, @p, @a, or a player name.",
            "请使用 @s、@p、@a 或玩家名。",
        ),
        BuildError::InvalidCoordinate => (
            "INVALID_COORDINATE",
            "Enter a finite coordinate within ±30000000.",
            "请输入 ±30000000 范围内的有限坐标。",
        ),
        BuildError::InvalidResourceId => (
            "INVALID_RESOURCE_ID",
            "Enter a simple namespaced ID, such as minecraft:torch.",
            "请输入简单的命名空间 ID，例如 minecraft:torch。",
        ),
        BuildError::InvalidQuantity => (
            "INVALID_QUANTITY",
            "Quantity must be from 1 through 64.",
            "数量须为 1 到 64。",
        ),
        BuildError::InvalidDuration => (
            "INVALID_DURATION",
            "Duration must be from 1 through 1000000 seconds.",
            "持续时间须为 1 到 1000000 秒。",
        ),
        BuildError::InvalidAmplifier => (
            "INVALID_AMPLIFIER",
            "Effect strength must be from 0 through 127.",
            "效果等级须为 0 到 127。",
        ),
    };
    input_error(locale, code, english, chinese)
}

// Keep generated WIT exports out of native unit-test builds; the same pure
// request handler remains covered natively, while plugin builds target wasm.
#[cfg(target_arch = "wasm32")]
mod component {
    wit_bindgen::generate!({path:"../../contracts/report",world:"report-plugin-context",generate_all});
    use exports::launcher::resource_report::reports::{Finding, Guest, Report, Severity};
    use launcher::resource_report::resources::{self, Failure, Input};
    struct CommandPocket;
    impl Guest for CommandPocket {
        fn analyze(_input: &Input) -> Result<Report, Failure> {
            if resources::cancelled() {
                return Err(Failure::Cancelled);
            }
            let zh = launcher::report_context::context::get().locale == "zh-CN";
            Ok(Report {
                schema_version: 1,
                findings: vec![Finding {
                    code: "command_pocket_frontend".into(),
                    severity: Severity::Info,
                    title: if zh { "指令口袋" } else { "Command Pocket" }.into(),
                    message: if zh {
                        "请从工具页面打开指令口袋。"
                    } else {
                        "Open Command Pocket from Tools."
                    }
                    .into(),
                    evidence: vec![],
                    suggestions: vec![],
                }],
                tables: vec![],
            })
        }
    }
    export!(CommandPocket);
}
#[cfg(test)]
mod tool_form {
    #[derive(Debug, PartialEq, Eq)]
    pub struct Failure {
        pub code: String,
        pub message: String,
    }
    #[derive(Debug, PartialEq, Eq)]
    pub struct TextBlock {
        pub label: Option<String>,
        pub text: String,
    }
    #[derive(Debug, PartialEq, Eq)]
    pub struct Output {
        pub title: String,
        pub blocks: Vec<TextBlock>,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_explicitly_supported_releases() {
        assert_eq!(JavaRelease::parse("1.20.1"), Ok(JavaRelease::V1_20_1));
        assert_eq!(JavaRelease::parse("1.21.1").unwrap().as_str(), "1.21.1");
        assert_eq!(JavaRelease::parse("1.21.4").unwrap().as_str(), "1.21.4");
        assert_eq!(
            JavaRelease::parse("1.21.5"),
            Err(BuildError::UnsupportedRelease)
        );
    }

    #[test]
    fn builds_coordinate_teleport_with_relative_values() {
        let result = build_command(
            JavaRelease::V1_20_1,
            &CommandTemplate::Teleport {
                target: Target::SelfPlayer,
                x: Coordinate::Relative(0.0),
                y: Coordinate::Absolute(64.0),
                z: Coordinate::Relative(-2.5),
            },
        );
        assert_eq!(result.unwrap(), "/tp @s ~ 64 ~-2.5");
    }

    #[test]
    fn builds_basic_give_and_effect_without_data_payloads() {
        assert_eq!(
            build_command(
                JavaRelease::V1_21_1,
                &CommandTemplate::Give {
                    target: Target::PlayerName("Alex_2".into()),
                    item_id: "minecraft:torch".into(),
                    quantity: 16,
                }
            )
            .unwrap(),
            "/give Alex_2 minecraft:torch 16"
        );
        assert_eq!(
            build_command(
                JavaRelease::V1_21_4,
                &CommandTemplate::Effect {
                    target: Target::NearestPlayer,
                    effect_id: "minecraft:speed".into(),
                    seconds: 30,
                    amplifier: 0,
                    hide_particles: false,
                }
            )
            .unwrap(),
            "/effect give @p minecraft:speed 30 0 false"
        );
    }

    #[test]
    fn rejects_raw_command_injection_and_invalid_fields() {
        assert_eq!(
            build_command(
                JavaRelease::V1_21_1,
                &CommandTemplate::Give {
                    target: Target::PlayerName("Alex\n/op @a".into()),
                    item_id: "minecraft:torch".into(),
                    quantity: 1,
                }
            ),
            Err(BuildError::InvalidTarget)
        );
        assert_eq!(
            build_command(
                JavaRelease::V1_21_1,
                &CommandTemplate::Give {
                    target: Target::SelfPlayer,
                    item_id: "minecraft:stone;op".into(),
                    quantity: 1,
                }
            ),
            Err(BuildError::InvalidResourceId)
        );
        assert_eq!(
            build_command(
                JavaRelease::V1_21_1,
                &CommandTemplate::Give {
                    target: Target::SelfPlayer,
                    item_id: "minecraft:stone".into(),
                    quantity: 65,
                }
            ),
            Err(BuildError::InvalidQuantity)
        );
        assert_eq!(
            build_command(
                JavaRelease::V1_21_1,
                &CommandTemplate::Teleport {
                    target: Target::SelfPlayer,
                    x: Coordinate::Absolute(f64::NAN),
                    y: Coordinate::Absolute(0.0),
                    z: Coordinate::Absolute(0.0),
                }
            ),
            Err(BuildError::InvalidCoordinate)
        );
        assert_eq!(
            build_command(
                JavaRelease::V1_21_1,
                &CommandTemplate::Effect {
                    target: Target::SelfPlayer,
                    effect_id: "minecraft:speed".into(),
                    seconds: 30,
                    amplifier: 128,
                    hide_particles: false,
                }
            ),
            Err(BuildError::InvalidAmplifier)
        );
    }

    fn form_input(template: &str) -> Value {
        serde_json::json!({
            "version": "1.21.4", "template": template, "target": "@s",
            "x": "~", "y": "64", "z": "~-2.5", "item_id": "minecraft:torch",
            "quantity": 16, "effect_id": "minecraft:speed", "seconds": 30,
            "amplifier": 0, "hide_particles": false
        })
    }

    #[test]
    fn tool_form_dispatch_generates_only_the_selected_template() {
        let teleport = build_input("en", &form_input("teleport")).unwrap();
        assert_eq!(teleport.title, "Generated command");
        assert_eq!(teleport.blocks[0].text, "/tp @s ~ 64 ~-2.5");
        let give = build_input("zh-CN", &form_input("give")).unwrap();
        assert_eq!(give.title, "生成的指令");
        assert_eq!(give.blocks[0].text, "/give @s minecraft:torch 16");
        let effect = build_input("en", &form_input("effect")).unwrap();
        assert_eq!(
            effect.blocks[0].text,
            "/effect give @s minecraft:speed 30 0 false"
        );
        assert_eq!(teleport.blocks[0].text.lines().count(), 1);
    }

    #[test]
    fn tool_form_rejects_unsupported_version_and_injected_coordinate_text() {
        let mut input = form_input("give");
        input["version"] = Value::String("1.21.5".into());
        assert_eq!(
            build_input("en", &input).unwrap_err().code,
            "UNSUPPORTED_RELEASE"
        );

        let mut input = form_input("teleport");
        input["x"] = Value::String("~\n/op @a".into());
        assert_eq!(
            build_input("en", &input).unwrap_err().code,
            "INVALID_COORDINATE"
        );
    }
}
