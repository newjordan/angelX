use std::cell::Cell;

thread_local! {
    static RIDE_COMPOSES: Cell<u64> = const { Cell::new(0) };
}

pub(super) fn note_ride_compose() {
    RIDE_COMPOSES.with(|count| count.set(count.get().saturating_add(1)));
}

pub(crate) fn take_ride_compose_count() -> u64 {
    RIDE_COMPOSES.with(|count| count.replace(0))
}
