use crate::CoreError;
use std::fmt::{Display, Formatter};

/// Stable, namespace-qualified content identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentId(String);

impl ContentId {
    pub fn new(value: impl Into<String>) -> Result<Self, CoreError> {
        let value = value.into();
        let Some((namespace, path)) = value.split_once(':') else {
            return Err(CoreError::InvalidId(value));
        };
        if namespace.is_empty()
            || path.is_empty()
            || !value.chars().all(|character| {
                character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || matches!(character, ':' | '_')
            })
        {
            return Err(CoreError::InvalidId(value));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ContentId {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

macro_rules! system_sequence_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name {
            pub system: ContentId,
            pub sequence: u64,
        }

        impl $name {
            #[must_use]
            pub fn new(system: ContentId, sequence: u64) -> Self {
                Self { system, sequence }
            }
        }
    };
}

system_sequence_id!(ProjectId);
system_sequence_id!(ShipId);
system_sequence_id!(PopulationId);

/// Stable identity for a fact source. Synthetic tick-zero observations cannot collide with ships.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObserverId {
    InitialOrigin(ContentId),
    Ship(ShipId),
}

impl ObserverId {
    #[must_use]
    pub fn ship_id(&self) -> Option<&ShipId> {
        match self {
            Self::InitialOrigin(_) => None,
            Self::Ship(ship_id) => Some(ship_id),
        }
    }
}

/// Observation identity is scoped to a typed stable observer and its never-reused sequence.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransmissionId {
    pub observer: ObserverId,
    pub sequence: u64,
}

/// Typed reservation ownership prevents equal numeric sequences in different domains colliding.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReservationOwner {
    Construction(ProjectId),
    Expedition(ShipId),
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SystemCounters {
    pub next_project_sequence: u64,
    pub next_ship_sequence: u64,
    pub next_population_sequence: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObserverCounters {
    pub next_transmission_sequence: u64,
}
