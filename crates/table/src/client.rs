//! The guest seat's client: holds the board as the server publishes
//! it (whole tree, then patches), for `angel_serve`'s `NativeLink`.
//! Drawing is the cockpit's business (it renders the board as text),
//! so `draw` is a no-op — this client exists to hold state and keep
//! the seat's command stream alive.

use crate::{Board, GAME, TableCmd};
use angel_net::client::{Frame, GameClient, View};
use angel_net::codec::{Bytes, Malformed};
use angel_net::doc::delta;
use angel_net::doc::value::Value;
use angel_net::doc::wire::Reader;
use angel_net::input::Hands;
use angel_net::proto::Welcome;

/// A guest's view of the table.
#[derive(Default)]
pub struct TableClient {
    names: Reader,
    tree: Option<Value>,
    /// The seat this client plays, from its welcome.
    pub seat: Option<u32>,
}

impl TableClient {
    /// The board as last published, once a snapshot has landed.
    pub fn board(&self) -> Option<Board> {
        self.tree.as_ref().and_then(Board::from_value)
    }
}

impl GameClient for TableClient {
    type Cmd = TableCmd;
    const GAME: &'static str = GAME;

    fn boot(welcome: &Welcome) -> Result<Self, String> {
        Ok(TableClient { seat: welcome.seat, ..TableClient::default() })
    }

    fn snapshot(&mut self, _tick: u32, whole: bool, body: &[u8]) -> Result<(), Malformed> {
        if whole {
            self.names = Reader::default();
            let mut b = Bytes::new(body);
            self.tree = Some(self.names.value(&mut b)?);
            b.end()?;
        } else if !body.is_empty() {
            let tree = self.tree.as_mut().ok_or(Malformed)?;
            let mut b = Bytes::new(body);
            let patch = self.names.patch(&mut b)?;
            b.end()?;
            delta::apply(tree, patch).map_err(|_| Malformed)?;
        }
        Ok(())
    }

    fn events(&mut self, _tick: u32, _events: &[&[u8]], _lines: &mut Vec<String>) {}

    fn command(&mut self, _hands: &Hands) -> TableCmd {
        TableCmd
    }

    fn encode_cmd(_cmd: &TableCmd, out: &mut Vec<u8>) {
        out.push(0);
    }

    fn draw(&mut self, _view: &View<TableCmd>, _frame: &mut Frame) {}

    fn controls(&self) -> &'static str {
        "{}"
    }
}
