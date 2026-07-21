use crate::{ContentId, CoreError};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::num::NonZeroU64;

/// Signed fixed-point coordinate measured in profile-defined integer quanta.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct FixedCoordinate(pub i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position3 {
    pub x: FixedCoordinate,
    pub y: FixedCoordinate,
    pub z: FixedCoordinate,
}

impl Position3 {
    #[must_use]
    pub const fn from_quanta(x: i64, y: i64, z: i64) -> Self {
        Self {
            x: FixedCoordinate(x),
            y: FixedCoordinate(y),
            z: FixedCoordinate(z),
        }
    }

    /// Checked exact squared coordinate distance. No square root or rounding is performed.
    pub fn checked_squared_distance(self, other: Self) -> Result<u128, CoreError> {
        fn square_difference(left: i64, right: i64) -> Result<u128, CoreError> {
            let difference = i128::from(left) - i128::from(right);
            difference
                .unsigned_abs()
                .checked_mul(difference.unsigned_abs())
                .ok_or(CoreError::Overflow)
        }
        let x = square_difference(self.x.0, other.x.0)?;
        let y = square_difference(self.y.0, other.y.0)?;
        let z = square_difference(self.z.0, other.z.0)?;
        x.checked_add(y)
            .and_then(|value| value.checked_add(z))
            .ok_or(CoreError::Overflow)
    }

    /// Checked integer distance, rounded toward the next coordinate quantum.
    pub fn checked_ceil_distance(self, other: Self) -> Result<u64, CoreError> {
        checked_ceil_sqrt(self.checked_squared_distance(other)?)
    }

    /// Jump eligibility is compared in squared space and therefore performs no rounding.
    pub fn checked_within_jump(self, other: Self, jump_range: u64) -> Result<bool, CoreError> {
        let range_squared = u128::from(jump_range)
            .checked_mul(u128::from(jump_range))
            .ok_or(CoreError::Overflow)?;
        Ok(self.checked_squared_distance(other)? <= range_squared)
    }
}

/// Returns the ceiling integer square root of an exact squared distance.
pub fn checked_ceil_sqrt(value: u128) -> Result<u64, CoreError> {
    if value == 0 {
        return Ok(0);
    }

    let mut low = 1_u128;
    let mut high = u128::from(u64::MAX);
    while low <= high {
        let midpoint = low + (high - low) / 2;
        match midpoint.cmp(&(value / midpoint)) {
            std::cmp::Ordering::Greater => high = midpoint - 1,
            _ => low = midpoint + 1,
        }
    }
    let floor = high;
    let root = if floor * floor == value {
        floor
    } else {
        floor.checked_add(1).ok_or(CoreError::Overflow)?
    };
    u64::try_from(root).map_err(|_| CoreError::Overflow)
}

/// A nonnegative rational rate, such as one tick per 500 coordinate quanta.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedRate {
    pub numerator: u64,
    pub denominator: NonZeroU64,
}

impl FixedRate {
    #[must_use]
    pub const fn new(numerator: u64, denominator: NonZeroU64) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    /// Applies this rate and rounds any nonzero fraction upward.
    pub fn checked_ceil(self, value: u64) -> Result<u64, CoreError> {
        let product = u128::from(value)
            .checked_mul(u128::from(self.numerator))
            .ok_or(CoreError::Overflow)?;
        let denominator = u128::from(self.denominator.get());
        let result = product
            .checked_add(denominator - 1)
            .ok_or(CoreError::Overflow)?
            / denominator;
        u64::try_from(result).map_err(|_| CoreError::Overflow)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteNode {
    pub system: ContentId,
    pub position: Position3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteLeg {
    pub from: ContentId,
    pub to: ContentId,
    pub distance: u64,
}

/// A route includes its source and target in `systems` and has one fewer leg.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    pub systems: Vec<ContentId>,
    pub legs: Vec<RouteLeg>,
    pub total_distance: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedRouteStop {
    /// Hidden intermediate stops are represented without disclosing their stable ID.
    pub system: Option<ContentId>,
    pub reached: bool,
}

/// Player-safe committed route. It exposes cost but never hidden intermediate identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedactedRoute {
    pub stops: Vec<RedactedRouteStop>,
    pub total_distance: u64,
}

impl Route {
    pub fn checked_duration(&self, speed: NonZeroU64) -> Result<u64, CoreError> {
        self.legs.iter().try_fold(0_u64, |duration, leg| {
            let leg_duration = FixedRate::new(1, speed).checked_ceil(leg.distance)?;
            duration
                .checked_add(leg_duration)
                .ok_or(CoreError::Overflow)
        })
    }

    pub fn checked_energy(&self, energy_rate: FixedRate) -> Result<u64, CoreError> {
        energy_rate.checked_ceil(self.total_distance)
    }

    /// Redacts only intermediate systems. Source and target are necessarily named by launch.
    #[must_use]
    pub fn redact(
        &self,
        identified: &BTreeSet<ContentId>,
        reached: &BTreeSet<ContentId>,
    ) -> Vec<RedactedRouteStop> {
        let last = self.systems.len().saturating_sub(1);
        self.systems
            .iter()
            .enumerate()
            .map(|(index, system)| {
                let reached_stop = reached.contains(system);
                let visible =
                    index == 0 || index == last || identified.contains(system) || reached_stop;
                RedactedRouteStop {
                    system: visible.then(|| system.clone()),
                    reached: reached_stop,
                }
            })
            .collect()
    }

    #[must_use]
    pub fn player_route(
        &self,
        identified: &BTreeSet<ContentId>,
        reached: &BTreeSet<ContentId>,
    ) -> RedactedRoute {
        RedactedRoute {
            stops: self.redact(identified, reached),
            total_distance: self.total_distance,
        }
    }
}

/// Finds a geometric route minimizing summed ceiling leg distance. Equal-cost routes use the
/// lexicographically lower complete stable-system-ID sequence.
pub fn shortest_route(
    nodes: &[RouteNode],
    source: &ContentId,
    target: &ContentId,
    jump_range: u64,
) -> Result<Option<Route>, CoreError> {
    let positions = nodes
        .iter()
        .map(|node| (node.system.clone(), node.position))
        .collect::<BTreeMap<_, _>>();
    if positions.len() != nodes.len()
        || !positions.contains_key(source)
        || !positions.contains_key(target)
    {
        return Ok(None);
    }
    if source == target {
        return Ok(Some(Route {
            systems: vec![source.clone()],
            legs: Vec::new(),
            total_distance: 0,
        }));
    }

    let mut best = BTreeMap::<ContentId, (u64, Vec<ContentId>)>::new();
    let initial_path = vec![source.clone()];
    best.insert(source.clone(), (0, initial_path.clone()));
    let mut candidates = BinaryHeap::new();
    candidates.push(Reverse((0_u64, initial_path, source.clone())));

    while let Some(Reverse((cost, path, current))) = candidates.pop() {
        if best.get(&current) != Some(&(cost, path.clone())) {
            continue;
        }
        if &current == target {
            return build_route(&positions, path).map(Some);
        }
        let current_position = positions[&current];
        for (next, next_position) in &positions {
            if next == &current || path.contains(next) {
                continue;
            }
            if !current_position.checked_within_jump(*next_position, jump_range)? {
                continue;
            }
            let distance = current_position.checked_ceil_distance(*next_position)?;
            let next_cost = cost.checked_add(distance).ok_or(CoreError::Overflow)?;
            let mut next_path = path.clone();
            next_path.push(next.clone());
            let replace = best
                .get(next)
                .is_none_or(|known| (next_cost, &next_path) < (known.0, &known.1));
            if replace {
                best.insert(next.clone(), (next_cost, next_path.clone()));
                candidates.push(Reverse((next_cost, next_path, next.clone())));
            }
        }
    }
    Ok(None)
}

fn build_route(
    positions: &BTreeMap<ContentId, Position3>,
    systems: Vec<ContentId>,
) -> Result<Route, CoreError> {
    let mut total_distance = 0_u64;
    let mut legs = Vec::with_capacity(systems.len().saturating_sub(1));
    for pair in systems.windows(2) {
        let distance = positions[&pair[0]].checked_ceil_distance(positions[&pair[1]])?;
        total_distance = total_distance
            .checked_add(distance)
            .ok_or(CoreError::Overflow)?;
        legs.push(RouteLeg {
            from: pair[0].clone(),
            to: pair[1].clone(),
            distance,
        });
    }
    Ok(Route {
        systems,
        legs,
        total_distance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squared_distance_is_exact_and_checked() {
        let origin = Position3::from_quanta(0, 0, 0);
        assert_eq!(
            origin.checked_squared_distance(Position3::from_quanta(3, 4, 12)),
            Ok(169)
        );
        assert_eq!(
            Position3::from_quanta(i64::MIN, i64::MIN, 0)
                .checked_squared_distance(Position3::from_quanta(i64::MAX, i64::MAX, 0)),
            Err(CoreError::Overflow)
        );
    }
}
