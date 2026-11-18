//! How many pages the card screen has: the knight's own cards, the realm's,
//! the book's and last the Herald's Bestiary. The controls turn these pages;
//! the cockpit draws them (`ui::viz::shooter_viz`).

use crate::together_shooter::{Book, Run};

pub const BOOK_PAGE: usize = 8;

pub const REALM_PAGE: usize = 1;

pub fn card_pages(run: &Run) -> usize {
    3 + book_pages(&run.book)
}

pub fn bestiary_page(run: &Run) -> usize {
    card_pages(run) - 1
}

pub fn book_pages(book: &Book) -> usize {
    book.cards.len().div_ceil(BOOK_PAGE)
}
