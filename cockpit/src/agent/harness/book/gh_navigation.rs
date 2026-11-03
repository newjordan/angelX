//! ⠣ gh — Volume II, the tool library: finding your way around the code —
//! outlines, directory listings, search, definitions, git status and the self
//! map.
//!
//! One section per tool; its pages are the sentences that told the model how
//! to use that tool and what each parameter means, verbatim, moved off the
//! schema. The schema keeps what the tool does and ends with the section's
//! route; each parameter's description is the address of its page. A tool
//! whose pages pass ten continues in a section named `<tool>, continued`.

use super::{Primary, Route, Sub};

pub(crate) const CELL: char = '⠣';

// The compile-time fallback uses the same catalog shipped for runtime reloads.
const CATALOG: &str = include_str!("../../../../book/gh_navigation.tsv");

const fn catalog_pages<const N: usize>(route: u8) -> [&'static str; N] {
    let mut pages = [""; N];
    let mut i = 0;
    while i < N {
        pages[i] = catalog_page(CATALOG, route, i);
        i += 1;
    }
    pages
}

const fn catalog_page(catalog: &'static str, route: u8, ordinal: usize) -> &'static str {
    let bytes = catalog.as_bytes();
    let mut start = 0;
    let mut seen = 0;
    while start < bytes.len() {
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'\n' {
            end += 1;
        }
        if end >= start + 2 && bytes[start] == route && bytes[start + 1] == b'\t' {
            if seen == ordinal {
                return catalog.split_at(start + 2).1.split_at(end - start - 2).0;
            }
            seen += 1;
        }
        start = end + 1;
    }
    panic!("missing navigation catalog page");
}

pub(crate) const PRIMARY: Primary = Primary {
    cell: CELL,
    name: "navigation",
    surface: "outlines, listings, search, definitions, status and the self map",
    subs: &[
        Sub {
            route: Route::new(CELL, '⠁'),
            name: "outline",
            signal: "using `outline`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<1>(b'1'),
        },
        Sub {
            route: Route::new(CELL, '⠃'),
            name: "list_dir",
            signal: "using `list_dir`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<7>(b'2'),
        },
        Sub {
            route: Route::new(CELL, '⠉'),
            name: "grep",
            signal: "using `grep`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<9>(b'3'),
        },
        Sub {
            route: Route::new(CELL, '⠙'),
            name: "find_files",
            signal: "using `find_files`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<3>(b'4'),
        },
        Sub {
            route: Route::new(CELL, '⠑'),
            name: "defs",
            signal: "using `defs`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<9>(b'5'),
        },
        Sub {
            route: Route::new(CELL, '⠋'),
            name: "git_status",
            signal: "using `git_status`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<1>(b'6'),
        },
        Sub {
            route: Route::new(CELL, '⠛'),
            name: "self_map",
            signal: "using `self_map`",
            action: "",
            ideas: "",
            pages: &catalog_pages::<4>(b'7'),
        },
    ],
};
