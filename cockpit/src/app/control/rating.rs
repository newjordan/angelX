use super::*;

impl App {
    pub(crate) fn rate_last_turn(&mut self, arg: Option<&str>) -> String {
        let requested = arg.unwrap_or("status").trim().to_ascii_lowercase();
        let Some(last) = self.last_completed_route.as_mut() else {
            return "no completed answer to rate yet · /rate useful|miss".to_string();
        };
        if matches!(requested.as_str(), "" | "status" | "show") {
            let route = last
                .route
                .model
                .as_deref()
                .unwrap_or(last.route.driver.as_str());
            return match last.verdict {
                Some(verdict) => format!("last answer: {route} · rated {}", verdict.label()),
                None => format!("last answer: {route} · unrated · /rate useful|miss"),
            };
        }
        let verdict = match requested.as_str() {
            "useful" | "good" | "up" | "+" | "yes" => {
                crate::knowledge::experience::RouteVerdict::Useful
            }
            "miss" | "bad" | "down" | "-" | "no" => {
                crate::knowledge::experience::RouteVerdict::Miss
            }
            _ => return "usage: /rate useful|miss (explicit last-answer label)".to_string(),
        };
        if let Some(existing) = last.verdict {
            return format!(
                "last answer already rated {} · one label per completed answer",
                existing.label()
            );
        }
        let route = last.route.clone();
        let completed_ms = last.completed_ms;
        crate::knowledge::experience::record_route_verdict(
            &route,
            verdict,
            completed_ms,
            self.tools.current_workspace(),
        );
        last.verdict = Some(verdict);
        self.route_evidence
            .apply_user_verdict(&route, verdict, completed_ms);
        format!(
            "last answer rated {} · route metadata only; no conversation text recorded",
            verdict.label()
        )
    }
}
