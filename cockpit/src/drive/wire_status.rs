//! Status words that a controller writes to disk and shows to a model as plain
//! strings. `wire_status!` declares the enum whose variants are the real states,
//! with each variant's exact word beside it, so the words cannot drift from the
//! states and a `match` over the enum must name every state.

/// Declares a status enum that serializes as one plain string.
///
/// The caller writes `fn allows(&self, next: &Self) -> bool` for the enum: the
/// table of legal moves, the way `drive::campaign::CampaignStatus::allows` does.
/// `step` is then the only way a stored status changes. Staying where you are is
/// not a move, and a word this build never wrote (an older or newer file, a hand
/// edit) is kept exactly as read so the file still loads and saves back
/// unchanged; it may move anywhere, as it could when it was a bare string.
macro_rules! wire_status {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident = $word:literal,)+
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        $vis enum $name {
            $($(#[$variant_meta])* $variant,)+
            /// A word no version of this program writes, kept verbatim.
            Unrecognised(String),
        }

        impl $name {
            /// The exact word written to disk and shown to a model.
            pub(crate) fn as_str(&self) -> &str {
                match self {
                    $(Self::$variant => $word,)+
                    Self::Unrecognised(word) => word,
                }
            }

            pub(crate) fn parse(word: &str) -> Self {
                match word {
                    $($word => Self::$variant,)+
                    _ => Self::Unrecognised(word.to_string()),
                }
            }

            /// Move to `next`, or say why not and change nothing.
            pub(crate) fn step(&mut self, next: Self) -> Result<(), String> {
                if *self != next
                    && !matches!(self, Self::Unrecognised(_))
                    && !self.allows(&next)
                {
                    return Err(format!(
                        "illegal {} move {} -> {}",
                        stringify!($name),
                        self,
                        next
                    ));
                }
                *self = next;
                Ok(())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Ok(Self::parse(&<String as serde::Deserialize>::deserialize(deserializer)?))
            }
        }
    };
}
