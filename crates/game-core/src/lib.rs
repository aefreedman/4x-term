//! Headless, deterministic Stage 4b simulation foundation.

mod ids;
mod knowledge;
mod population;
mod resources;
mod routing;
mod ships;
mod simulation;
mod world;

pub use ids::*;
pub use knowledge::*;
pub use population::*;
pub use resources::*;
pub use routing::*;
pub use ships::*;
pub use simulation::*;
pub use world::*;

use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CoreError {
    #[error("invalid content id: {0}")]
    InvalidId(String),
    #[error("checked arithmetic overflow")]
    Overflow,
    #[error("duplicate resource id: {0}")]
    DuplicateResourceId(ContentId),
    #[error("duplicate location id: {0}")]
    DuplicateLocationId(ContentId),
    #[error("duplicate system definition for location: {0}")]
    DuplicateSystemLocation(ContentId),
    #[error("duplicate community id: {0}")]
    DuplicateCommunityId(ContentId),
    #[error("duplicate reclaimable site id: {0}")]
    DuplicateSiteId(ContentId),
    #[error("duplicate body id: {0}")]
    DuplicateBodyId(ContentId),
    #[error("duplicate slot id {slot} on body {body}")]
    DuplicateSlotId { body: ContentId, slot: ContentId },
    #[error("duplicate development id: {0}")]
    DuplicateDevelopmentId(ContentId),
    #[error("duplicate population id")]
    DuplicatePopulationId,
    #[error("new-world definitions cannot contain initial population tokens")]
    InitialPopulationTokensNotAllowed,
    #[error("neutral system has a community: {0}")]
    NeutralSystemHasCommunity(ContentId),
    #[error("founded remote system must have exactly one community: {0}")]
    FoundedSystemMissingCommunity(ContentId),
    #[error("runtime population/transit bijection is invalid: {0}")]
    InvalidTransitPopulationBijection(String),
    #[error("origin references unknown system: {0}")]
    UnknownOriginSystem(ContentId),
    #[error("origin references unknown community: {0}")]
    UnknownOriginCommunity(ContentId),
    #[error("origin community does not belong to the origin system")]
    OriginCommunitySystemMismatch,
    #[error("more than one community belongs to system: {0}")]
    DuplicateCommunitySystem(ContentId),
    #[error("community {community} references unknown system: {system}")]
    UnknownCommunitySystem {
        community: ContentId,
        system: ContentId,
    },
    #[error("system references unknown location: {0}")]
    UnknownSystemLocation(ContentId),
    #[error("not every location has persistent system state")]
    MissingPersistentSystem,
    #[error("system strength must be nonzero: {0}")]
    InvalidSystemStrength(ContentId),
    #[error("site {site} references unknown location: {location}")]
    UnknownSiteLocation {
        site: ContentId,
        location: ContentId,
    },
    #[error("system {location} stocks reference unknown resource: {resource}")]
    UnknownSystemStockResource {
        location: ContentId,
        resource: ContentId,
    },
    #[error("body {body} references unknown resource: {resource}")]
    UnknownBodyResource {
        body: ContentId,
        resource: ContentId,
    },
    #[error("body {body} has invalid natural resource quantity/type: {resource}")]
    InvalidBodyResource {
        body: ContentId,
        resource: ContentId,
    },
    #[error("unknown system: {0}")]
    UnknownSystem(ContentId),
    #[error("system is not directly commandable: {0}")]
    SystemNotCommandable(ContentId),
    #[error("cannot unlock commands for neutral system: {0}")]
    CannotUnlockNeutralSystem(ContentId),
    #[error("knowledge integration failed: {0}")]
    KnowledgeIntegration(String),
    #[error("unknown body: {0}")]
    UnknownBody(ContentId),
    #[error("unknown slot {slot} on body {body}")]
    UnknownDevelopmentSlot { body: ContentId, slot: ContentId },
    #[error("development slot {body}/{slot} is occupied or reserved")]
    DevelopmentSlotUnavailable { body: ContentId, slot: ContentId },
    #[error("Extractor construction requires a body-resource target")]
    ExtractorTargetRequired,
    #[error("only an Extractor may have a body-resource target")]
    UnexpectedExtractorTarget,
    #[error("incompatible Extractor target {body}/{resource}")]
    IncompatibleExtractorTarget {
        body: ContentId,
        resource: ContentId,
    },
    #[error("unknown project: {0:?}")]
    UnknownProject(ProjectId),
    #[error("invalid Shipyard project: {0:?}")]
    InvalidShipProject(ProjectId),
    #[error("Shipyard project has already begun and cannot be cancelled: {0:?}")]
    ShipProjectAlreadyBegun(ProjectId),
    #[error("slot {body}/{slot} is not a functional Shipyard")]
    NotFunctionalShipyard { body: ContentId, slot: ContentId },
    #[error("unknown completed ship asset: {0:?}")]
    UnknownCompletedShip(ShipId),
    #[error("completed ship asset has the wrong kind: {0:?}")]
    WrongCompletedShipKind(ShipId),
    #[error("completed ship asset is not available until after its completion tick: {0:?}")]
    CompletedShipNotReady(ShipId),
    #[error("ship target must be distinct from source: {0}")]
    ShipTargetMustBeDistinct(ContentId),
    #[error("system is not targetable with current knowledge: {0}")]
    SystemNotTargetable(ContentId),
    #[error("invalid probe jump limit {requested}; authored maximum is {maximum}")]
    InvalidProbeJumpLimit { requested: u64, maximum: u64 },
    #[error("no route from {from_system} to {target} within jump limit {jump_limit}")]
    NoShipRoute {
        from_system: ContentId,
        target: ContentId,
        jump_limit: u64,
    },
    #[error("complete target knowledge requires two slot reservations: {0}")]
    CompleteKnowledgeRequiresReservations(ContentId),
    #[error("summary target knowledge cannot name slot reservations: {0}")]
    SummaryKnowledgeCannotReserve(ContentId),
    #[error("complete target knowledge lacks the named slot facts: {0}")]
    IncompleteTargetSlotKnowledge(ContentId),
    #[error("invalid expedition reservation: {0}")]
    InvalidExpeditionReservation(String),
    #[error("source system has no resident population available for departure: {0}")]
    NoResidentPopulation(ContentId),
    #[error("unknown in-transit population: {0:?}")]
    UnknownTransitPopulation(PopulationId),
    #[error("population is not carried by the resolving expedition: {0:?}")]
    InvalidTransitPopulation(PopulationId),
    #[error("immutable map/runtime body or slot shape mismatch for system: {0}")]
    MapRuntimeMismatch(ContentId),
    #[error("construction has already begun and cannot be cancelled: {0:?}")]
    ConstructionAlreadyBegun(ProjectId),
    #[error("invalid slot reservation for construction project: {0:?}")]
    InvalidConstructionReservation(ProjectId),
    #[error("available Energy {available} exceeds capacity {capacity}")]
    EnergyAboveCapacity { available: u64, capacity: u64 },
    #[error("population ID references unknown birth system: {0}")]
    UnknownPopulationBirthSystem(ContentId),
    #[error("unknown population community: {0}")]
    UnknownPopulationCommunity(ContentId),
    #[error("population references missing/nonfunctional Habitat: {0}")]
    UnknownPopulationHabitat(ContentId),
    #[error("Habitat is referenced by more than one population: {0}")]
    HabitatMultiplyOccupied(ContentId),
    #[error("world tuning references unknown resource: {0}")]
    UnknownEngineResource(ContentId),
    #[error("invalid world tuning: {0}")]
    InvalidTuning(String),
    #[error("invalid {role:?} construction recipe: {reason}")]
    InvalidConstructionRecipe {
        role: DevelopmentRole,
        reason: String,
    },
    #[error("resource transfers must move a nonzero quantity")]
    ZeroResourceTransfer,
    #[error("resource transfer references unknown resource: {0}")]
    UnknownTransferResource(ContentId),
    #[error("insufficient {resource}: available {available}, requested {requested}")]
    InsufficientResource {
        resource: ContentId,
        available: u64,
        requested: u64,
    },
}
