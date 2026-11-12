use angel_live::realm::HostCx;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Building {
    Keep,
    Gatehouse,
    Rookery,
    Scriptorium,
    Smithy,
    Chapel,
    RoundTable,
    Observatory,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Herald {
    texts: [String; 2],
    leads: [usize; 2],
}

impl Herald {
    pub fn new(running: String, settled: String, running_lead: usize, settled_lead: usize) -> Self {
        Self {
            texts: [running, settled],
            leads: [running_lead, settled_lead],
        }
    }

    pub fn literal(name: &str, args: &str) -> Self {
        let line = if args.is_empty() {
            format!("▸ {name}")
        } else {
            format!("▸ {name} {args}")
        };
        Self::new(line.clone(), line, 0, 0)
    }

    pub fn text(&self, settled: bool) -> &str {
        &self.texts[usize::from(settled)]
    }

    pub fn lead_len(&self, settled: bool) -> usize {
        self.leads[usize::from(settled)]
    }
}

pub enum Step {
    Say(String),
    Help(String),
    Ask(String),
    ResetYaw,
    ShowWorld,
    ReturnToWorld,
    OpenRealm,
    Saddle(Building),
    FocusStage,
    Redraw(&'static str),
    Collapse,
}

pub trait CommandsWorld {
    fn herald(_name: &str, _args: &str) -> Option<Herald>
    where
        Self: Sized,
    {
        None
    }

    fn map_enabled() -> bool
    where
        Self: Sized,
    {
        true
    }

    fn together_context(&self) -> String {
        String::new()
    }

    fn turn_opened(&mut self) {}

    fn workshop_opened(&mut self) {}

    fn lesson_opened(&mut self, _topic: Option<&str>) -> Option<Building> {
        None
    }
}

pub trait CommandsRules {
    fn world_command(
        &mut self,
        _cx: &mut dyn HostCx,
        _arg: Option<&str>,
        _saddled: bool,
        _saddled_on_quest: bool,
    ) -> Vec<Step> {
        Vec::new()
    }

    fn together_command(&mut self, _cx: &mut dyn HostCx, _arg: Option<&str>) -> Vec<Step> {
        Vec::new()
    }

    fn village_command(&mut self) -> Vec<Step> {
        Vec::new()
    }
}
