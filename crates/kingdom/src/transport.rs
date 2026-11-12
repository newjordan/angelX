use super::*;
use crate::economy::{add, capped_add, neighbours, take};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

impl Kingdom {
    fn delivery_request(&self, c: &Catalog, b: &Building) -> Option<(Stock, Stock)> {
        if c.plans[&b.plan]
            .power
            .as_ref()
            .is_some_and(|p| p.is_source())
        {
            self.source_fuel(c, b).map(|fuel| (fuel, Stock::new()))
        } else {
            self.job(c, b).map(|(input, output, _)| (input, output))
        }
    }
    /// Shortest walk from a loading dock or road tile to the destination dock.
    /// A removed road can strand a cart; restoring a connection resumes its trip.
    pub fn route(&self, from: Tile, to: u32) -> Option<Vec<Tile>> {
        let destination = self.buildings.get(&to)?.tile;
        if from == destination {
            return Some(vec![from]);
        }
        let docks: BTreeSet<_> = neighbours(destination)
            .into_iter()
            .filter(|t| self.roads.contains(t))
            .collect();
        let mut parents = BTreeMap::from([(from, from)]);
        let mut queue = VecDeque::from([from]);
        while let Some(tile) = queue.pop_front() {
            if docks.contains(&tile) {
                let mut path = vec![destination, tile];
                let mut cursor = tile;
                while cursor != from {
                    cursor = parents[&cursor];
                    path.push(cursor);
                }
                path.reverse();
                return Some(path);
            }
            for next in neighbours(tile) {
                if self.roads.contains(&next) && !parents.contains_key(&next) {
                    parents.insert(next, tile);
                    queue.push_back(next);
                }
            }
        }
        None
    }
    fn reserved(&self, to: u32, resource: &str) -> u32 {
        self.shipments
            .iter()
            .filter(|s| s.to == to && s.resource == resource)
            .map(|s| s.amount)
            .sum()
    }
    pub fn in_transit(&self, resource: &str) -> u32 {
        self.shipments
            .iter()
            .filter(|s| s.resource == resource)
            .map(|s| s.amount)
            .sum()
    }
    /// Counts delivered, buffered and travelling material for conservation checks
    /// and the supply ledger. Construction spends only delivered camp stock.
    pub fn total(&self, resource: &str) -> u32 {
        self.amount(resource)
            + self.in_transit(resource)
            + self
                .buildings
                .values()
                .map(|b| {
                    b.input.get(resource).copied().unwrap_or(0)
                        + b.output.get(resource).copied().unwrap_or(0)
                })
                .sum::<u32>()
    }
    pub(crate) fn transport(&mut self, c: &Catalog) {
        let capacity = self.report(c).capacity;
        let mut moving = std::mem::take(&mut self.shipments);
        for cart in &mut moving {
            // Demolition, pausing or recipe changes never destroy cargo. Goods
            // no longer wanted by a workshop return to the camp.
            if cart.to != 1
                && self.buildings.get(&cart.to).is_none_or(|b| {
                    b.paused
                        || self
                            .delivery_request(c, b)
                            .is_none_or(|(input, _)| !input.contains_key(&cart.resource))
                })
            {
                cart.to = 1;
            }
            if let Some(path) = self.route(cart.tile, cart.to) {
                if path.len() > 1 {
                    cart.tile = path[1];
                }
            }
            let at_destination = self
                .buildings
                .get(&cart.to)
                .is_some_and(|b| b.tile == cart.tile);
            if at_destination {
                if cart.to == 1 {
                    if self.amount(&cart.resource) + cart.amount <= capacity {
                        add(&mut self.stock, &cart.resource, cart.amount);
                        capped_add(&mut self.delivered, &cart.resource, cart.amount);
                        cart.amount = 0;
                    }
                } else if let Some(b) = self.buildings.get_mut(&cart.to) {
                    if b.input.get(&cart.resource).copied().unwrap_or(0) + cart.amount <= 32 {
                        add(&mut b.input, &cart.resource, cart.amount);
                        cart.amount = 0;
                    }
                }
            }
        }
        moving.retain(|s| s.amount > 0);
        self.shipments = moving;
        let slots = self.report(c).carriers.min(256) as usize;
        let connected = self.connected();
        // Rotate loading priority so a busy early workshop cannot monopolise
        // every carrier. Direct producer-to-consumer routes precede storage.
        let mut ids: Vec<_> = self.buildings.keys().copied().collect();
        ids.sort_by_key(|id| {
            (
                std::cmp::Reverse(self.buildings[id].priority),
                *id <= self.dispatch_cursor,
                *id,
            )
        });
        for &to in &ids {
            if self.shipments.len() >= slots {
                break;
            }
            let b = &self.buildings[&to];
            if b.paused || b.construction > 0 || !connected.contains(&to) {
                continue;
            }
            let Some((inputs, outputs)) = self.delivery_request(c, b) else {
                continue;
            };
            if self.target_met(b, &outputs) {
                continue;
            }
            for (r, n) in inputs {
                if self.shipments.len() >= slots {
                    break;
                }
                if r == "food"
                    && self.amount("food") + self.reserved(1, "food")
                        < self.food_delivery_target(c).min(capacity)
                {
                    continue;
                }
                let have =
                    self.buildings[&to].input.get(&r).copied().unwrap_or(0) + self.reserved(to, &r);
                let wanted = (n * 2).max(4).min(24).saturating_sub(have);
                if wanted == 0 {
                    continue;
                }
                let mut sources: Vec<_> = self
                    .buildings
                    .iter()
                    .filter(|(id, b)| {
                        **id != to && **id != 1 && b.output.get(&r).copied().unwrap_or(0) > 0
                    })
                    .filter_map(|(&id, b)| self.route(b.tile, to).map(|p| (p.len(), id)))
                    .collect();
                sources.sort();
                sources.push((usize::MAX, 1));
                for (_, from) in sources {
                    if self.dispatch(c, from, to, &r, wanted) {
                        self.dispatch_cursor = to;
                        break;
                    }
                }
            }
        }
        for &from in &ids {
            if self.shipments.len() >= slots {
                break;
            }
            if from == 1 {
                continue;
            }
            let output = self.buildings[&from].output.clone();
            for (r, _) in output {
                if self.shipments.len() >= slots {
                    break;
                }
                let room = capacity.saturating_sub(self.amount(&r) + self.reserved(1, &r));
                if room > 0 && self.dispatch(c, from, 1, &r, room) {
                    self.dispatch_cursor = from;
                }
            }
        }
    }
    fn dispatch(&mut self, c: &Catalog, from: u32, to: u32, r: &str, wanted: u32) -> bool {
        let tile = self.buildings[&from].tile;
        if self.route(tile, to).is_none() {
            return false;
        }
        let reserve = if from == 1 && to != 1 && r == "food" {
            self.food_delivery_target(c)
        } else {
            0
        };
        let source = if from == 1 {
            &mut self.stock
        } else {
            &mut self.buildings.get_mut(&from).unwrap().output
        };
        let n = wanted
            .min(4)
            .min(source.get(r).copied().unwrap_or(0).saturating_sub(reserve));
        if n == 0 {
            return false;
        }
        take(source, &Stock::from([(r.into(), n)]));
        self.shipments.push(Shipment {
            resource: r.into(),
            amount: n,
            from,
            to,
            tile,
        });
        true
    }
}
