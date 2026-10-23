// SPDX-License-Identifier: MIT
//! Original pure-text session card composer. The plugin has no send capability.
wit_bindgen::generate!({ path: "../../contracts/message-composer", world: "message-composer" });

use exports::launcher::message_composer::composer::{
    Draft, Failure, Guest, Request, SelectedSession,
};

struct SessionCard;

fn failure(locale: &str, code: &str, en: &str, zh: &str) -> Failure {
    Failure {
        code: code.into(),
        message: if locale == "zh-CN" { zh } else { en }.into(),
    }
}

fn context_text(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(char::is_control)
}

fn compose(
    context_locale: &str,
    session: Option<SelectedSession>,
    note: &str,
) -> Result<Draft, Failure> {
    let Some(session) = session else {
        return Err(failure(
            context_locale,
            "SESSION_REQUIRED",
            "Select a game session first.",
            "请先选择游戏。",
        ));
    };
    if !matches!(context_locale, "en" | "zh-CN")
        || note.len() > 512
        || note
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\n' | '\r'))
    {
        return Err(failure(
            context_locale,
            "INVALID_REQUEST",
            "The session card request is invalid.",
            "分享卡片内容无效。",
        ));
    }
    if !context_text(&session.game_name)
        || !context_text(&session.game_version)
        || session
            .loader
            .as_deref()
            .is_some_and(|loader| !context_text(loader))
    {
        return Err(failure(
            context_locale,
            "INVALID_REQUEST",
            "The selected game details are invalid.",
            "所选游戏的信息无效。",
        ));
    }
    let mut text = if context_locale == "zh-CN" {
        format!(
            "一起玩 {}\n版本：{}",
            session.game_name.trim(),
            session.game_version.trim()
        )
    } else {
        format!(
            "Join me in {}\nVersion: {}",
            session.game_name.trim(),
            session.game_version.trim()
        )
    };
    if let Some(loader) = session.loader {
        text.push_str(&format!(" · {}", loader.trim()));
    }
    // Normalize line endings, retain intentional multiline notes, omit an empty note.
    let note = note.replace("\r\n", "\n");
    if note.contains('\r') {
        return Err(failure(
            context_locale,
            "INVALID_REQUEST",
            "The session card request is invalid.",
            "分享卡片内容无效。",
        ));
    }
    if !note.trim().is_empty() {
        text.push('\n');
        text.push_str(note.trim());
    }
    if text.trim().is_empty() || text.len() > 1024 {
        return Err(failure(
            context_locale,
            "DRAFT_LIMIT",
            "The session card is too long.",
            "分享卡片过长。",
        ));
    }
    Ok(Draft { text })
}

impl Guest for SessionCard {
    fn compose(request: Request) -> Result<Draft, Failure> {
        compose(
            &request.context.locale,
            request.context.selected_session,
            &request.note,
        )
    }
}

export!(SessionCard);

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> SelectedSession {
        SelectedSession {
            game_name: "Minecraft".into(),
            game_version: "1.21.4".into(),
            loader: Some("Fabric".into()),
        }
    }

    #[test]
    fn english_session_card_uses_selected_session_and_note() {
        let draft = compose("en", Some(session()), "Casual survival, bring your builds.").unwrap();
        assert_eq!(
            draft.text,
            "Join me in Minecraft\nVersion: 1.21.4 · Fabric\nCasual survival, bring your builds."
        );
    }

    #[test]
    fn chinese_session_card_uses_selected_session_and_note() {
        let draft = compose("zh-CN", Some(session()), "轻松生存，欢迎带上你的建筑。").unwrap();
        assert_eq!(
            draft.text,
            "一起玩 Minecraft\n版本：1.21.4 · Fabric\n轻松生存，欢迎带上你的建筑。"
        );
    }

    #[test]
    fn requires_session_and_bounds_input() {
        assert_eq!(
            compose("en", None, "note").unwrap_err().code,
            "SESSION_REQUIRED"
        );
        assert_eq!(
            compose("en", Some(session()), &"a".repeat(513))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
        let oversized = SelectedSession {
            game_name: "a".repeat(1100),
            ..session()
        };
        assert_eq!(
            compose("en", Some(oversized), "").unwrap_err().code,
            "DRAFT_LIMIT"
        );
    }

    #[test]
    fn vanilla_empty_note_has_no_placeholder_or_join_link() {
        let selected = SelectedSession {
            loader: None,
            ..session()
        };
        let text = compose("en", Some(selected), " \r\n ").unwrap().text;
        assert_eq!(text, "Join me in Minecraft\nVersion: 1.21.4");
        assert!(!text.ends_with('\n'));
        assert!(!text.contains("http"));
    }

    #[test]
    fn unicode_and_multiline_notes_are_preserved_with_byte_limits() {
        let text = compose("zh-CN", Some(session()), "  周五见！\r\n带上你的建筑 🏡  ")
            .unwrap()
            .text;
        assert!(text.ends_with("周五见！\n带上你的建筑 🏡"));
        let note = "🏡".repeat(128);
        assert!(
            compose("zh-CN", Some(session()), &note)
                .unwrap()
                .text
                .ends_with(&note)
        );
        assert_eq!(
            compose("zh-CN", Some(session()), &(note + "a"))
                .unwrap_err()
                .code,
            "INVALID_REQUEST"
        );
    }

    #[test]
    fn localized_errors_and_single_line_context() {
        assert_eq!(
            compose("zh-CN", None, "").unwrap_err().message,
            "请先选择游戏。"
        );
        for name in ["   ", "Minecraft\nJoin: fake"] {
            assert_eq!(
                compose(
                    "en",
                    Some(SelectedSession {
                        game_name: name.into(),
                        ..session()
                    }),
                    ""
                )
                .unwrap_err()
                .code,
                "INVALID_REQUEST"
            );
        }
    }
}
