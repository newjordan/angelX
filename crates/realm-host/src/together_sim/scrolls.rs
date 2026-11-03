use super::*;
use angel_delve::rules::scrolls::{self, Capture, Intent, Prepared, ResultValue};
use serde::Deserialize;

pub(super) fn prepare_scroll(
    engine: Option<&mut angel_live::system::System>,
    run: &Run,
    player: u32,
    intent: &Intent,
) -> Result<Prepared, String> {
    let capture = Capture::new(run, player, intent)?;
    let result = if let Some(engine) = engine {
        let parsed = std::cell::RefCell::new(None);
        engine.compute(
            scrolls::CONTRACT,
            scrolls::API,
            serde_json::to_value(&capture.request).map_err(|e| e.to_string())?,
            |value| {
                let result = ResultValue::deserialize(value).map_err(|e| e.to_string())?;
                capture.request.validate_result(&result)?;
                parsed.replace(Some(result));
                Ok(())
            },
        )?;
        parsed
            .into_inner()
            .ok_or("scroll policy result was not validated")?
    } else {
        capture.request.evaluate()?
    };
    capture.prepare(run, result)
}
