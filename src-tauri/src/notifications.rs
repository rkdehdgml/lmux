use crate::events::Live;

/// 창이 활성이 아닐 때 알림을 보낼지 판단한다.
/// `prev`는 이전 live 상태, `next`는 새 live 상태.
/// Waiting 또는 Done으로 **새로** 전환됐을 때만 true를 반환한다.
/// 같은 상태 반복 또는 Working 전환에는 보내지 않는다.
pub fn should_notify(prev: Option<&Live>, next: Option<&Live>) -> bool {
    match next {
        Some(Live::Waiting { .. }) => !matches!(prev, Some(Live::Waiting { .. })),
        Some(Live::Done) => !matches!(prev, Some(Live::Done)),
        _ => false,
    }
}

/// 알림 제목 문자열. 세션 이름과 상태만 담는다.
pub fn notification_title(session_name: &str, live: &Live) -> String {
    let state = match live {
        Live::Waiting { .. } => "승인 대기",
        Live::Done => "완료",
        Live::Working => "작업 중",
    };
    format!("{session_name} — {state}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use Live::{Done, Waiting, Working};

    fn waiting() -> Live {
        Waiting {
            request: "tool1".into(),
        }
    }

    #[test]
    fn notifies_on_new_waiting() {
        assert!(should_notify(None, Some(&waiting())));
        assert!(should_notify(Some(&Working), Some(&waiting())));
        assert!(should_notify(Some(&Done), Some(&waiting())));
    }

    #[test]
    fn does_not_notify_on_repeated_waiting() {
        assert!(!should_notify(Some(&waiting()), Some(&waiting())));
    }

    #[test]
    fn notifies_on_new_done() {
        assert!(should_notify(None, Some(&Done)));
        assert!(should_notify(Some(&Working), Some(&Done)));
        assert!(should_notify(Some(&waiting()), Some(&Done)));
    }

    #[test]
    fn does_not_notify_on_repeated_done() {
        assert!(!should_notify(Some(&Done), Some(&Done)));
    }

    #[test]
    fn does_not_notify_on_working_or_none() {
        assert!(!should_notify(None, Some(&Working)));
        assert!(!should_notify(Some(&waiting()), Some(&Working)));
        assert!(!should_notify(Some(&Done), None));
        assert!(!should_notify(None, None));
    }

    #[test]
    fn notification_title_format() {
        assert_eq!(
            notification_title("API 세션", &waiting()),
            "API 세션 — 승인 대기"
        );
        assert_eq!(notification_title("API 세션", &Done), "API 세션 — 완료");
    }
}
