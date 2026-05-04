//! Centralized identifiers for the DSL keywords that show up in more than one
//! parser module.
//!
//! Adding `capture`/`validate`/`followed`/`not`/`by` here in one place keeps
//! the canonical spellings from drifting across `pattern.rs`, `dynamic.rs`, and
//! lookahead parsing.

pub(crate) const FOLLOWED: &str = "followed";
pub(crate) const NOT: &str = "not";
pub(crate) const BY: &str = "by";
pub(crate) const CAPTURE: &str = "capture";
pub(crate) const VALIDATE: &str = "validate";

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum DslKeyword {
    Followed,
    Not,
    By,
    Capture,
    Validate,
}

impl DslKeyword {
    pub(crate) fn from_ident(ident: &syn::Ident) -> Option<Self> {
        match ident.to_string().as_str() {
            FOLLOWED => Some(Self::Followed),
            NOT => Some(Self::Not),
            BY => Some(Self::By),
            CAPTURE => Some(Self::Capture),
            VALIDATE => Some(Self::Validate),
            _ => None,
        }
    }

    #[cfg(feature = "dynamic-state")]
    pub(crate) fn starts_lookahead(self) -> bool {
        matches!(self, Self::Followed | Self::Not)
    }

    pub(crate) fn is_dynamic_op(self) -> bool {
        matches!(self, Self::Capture | Self::Validate)
    }
}

impl<'a> TryFrom<&'a syn::Ident> for DslKeyword {
    type Error = ();

    fn try_from(ident: &'a syn::Ident) -> Result<Self, Self::Error> {
        Self::from_ident(ident).ok_or(())
    }
}
