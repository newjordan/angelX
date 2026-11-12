use crate::console::{NO_REALM, Shelf};
use angel_live::realm::HostCx;
use angel_plug::commands::{CommandsRules, CommandsWorld, Step};

impl CommandsWorld for Shelf {}

impl CommandsRules for Shelf {
    fn world_command(
        &mut self,
        _cx: &mut dyn HostCx,
        _arg: Option<&str>,
        _saddled: bool,
        _saddled_on_quest: bool,
    ) -> Vec<Step> {
        vec![Step::Say(NO_REALM.to_string())]
    }

    fn together_command(&mut self, _cx: &mut dyn HostCx, _arg: Option<&str>) -> Vec<Step> {
        vec![Step::Say(NO_REALM.to_string())]
    }

    fn village_command(&mut self) -> Vec<Step> {
        vec![Step::Say(NO_REALM.to_string())]
    }
}
