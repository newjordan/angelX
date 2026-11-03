//! How many pages the card screen has: the knight's own cards, the realm's,
//! the book's and last the Herald's Bestiary. The controls turn these pages;
//! the cockpit draws them (`ui::viz::shooter_viz`).

use crate::together_shooter::{Book, Run};

/// Cards of the book on one page.
pub const BOOK_PAGE: usize = 8;

/// The card screen's second page: the treasury and the wishing stone.
pub const REALM_PAGE: usize = 1;

/// Pages of the card screen: your cards, the realm, the book, and last the
/// Herald's Bestiary.
pub fn card_pages(run: &Run) -> usize {
    3 + book_pages(&run.book)
}

/// The Bestiary's page: the last.
pub fn bestiary_page(run: &Run) -> usize {
    card_pages(run) - 1
}

/// Pages of a book's card screen past the knight's own and the realm's.
pub fn book_pages(book: &Book) -> usize {
    book.cards.len().div_ceil(BOOK_PAGE)
}
