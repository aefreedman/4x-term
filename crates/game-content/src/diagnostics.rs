use std::fmt::{Display, Formatter};
use thiserror::Error;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContentDiagnostic {
    pub source: String,
    pub definition: String,
    pub field: String,
    pub message: String,
}

impl Display for ContentDiagnostic {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}:{}:{}: {}",
            self.source, self.definition, self.field, self.message
        )
    }
}

#[derive(Debug, Error)]
#[error("content compilation failed:\n{}", .0.iter().map(ToString::to_string).collect::<Vec<_>>().join("\n"))]
pub struct ContentErrors(pub Vec<ContentDiagnostic>);

impl ContentErrors {
    #[must_use]
    pub fn diagnostics(&self) -> &[ContentDiagnostic] {
        &self.0
    }
    pub(crate) fn one(
        source: String,
        definition: impl Into<String>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self(vec![ContentDiagnostic {
            source,
            definition: definition.into(),
            field: field.into(),
            message: message.into(),
        }])
    }
}

pub(crate) fn push(
    diagnostics: &mut Vec<ContentDiagnostic>,
    source: &str,
    definition: impl Into<String>,
    field: impl Into<String>,
    message: impl Into<String>,
) {
    diagnostics.push(ContentDiagnostic {
        source: source.into(),
        definition: definition.into(),
        field: field.into(),
        message: message.into(),
    });
}
