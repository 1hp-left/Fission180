//! Transactional entity editing and simple closed-chain region extraction.
use crate::{
    EPS, Profile, Result, Sketch,
    expression::Parameters,
    invalid,
    sketch::{Constraint, Entity, SketchGraph},
};
use std::collections::{BTreeMap, BTreeSet};
impl Constraint {
    pub fn references(&self) -> Vec<u64> {
        match self {
            Self::Fixed { point, .. } => vec![*point],
            Self::Coincident { a, b }
            | Self::Concentric { a, b }
            | Self::DistanceX { a, b, .. }
            | Self::DistanceY { a, b, .. }
            | Self::Equal { a, b }
            | Self::Parallel { a, b }
            | Self::Perpendicular { a, b } => vec![*a, *b],
            Self::Midpoint { point, line } => vec![*point, *line],
            Self::Horizontal { line } | Self::Vertical { line } | Self::Length { line, .. } => {
                vec![*line]
            }
            Self::Radius { circle, .. } => vec![*circle],
            Self::Tangent { line, circle } => vec![*line, *circle],
        }
    }
}
impl SketchGraph {
    pub(crate) fn loop_points(&self, entities: &[u64]) -> Result<Vec<[f64; 2]>> {
        let Some(Entity::Line {
            a: start,
            b: second,
        }) = entities.first().and_then(|id| self.entities.get(id))
        else {
            return invalid("Region loop must contain lines");
        };
        let mut result = vec![self.points[start]];
        let mut cursor = *second;
        for id in &entities[1..] {
            result.push(self.points[&cursor]);
            cursor = match self.entities.get(id) {
                Some(Entity::Line { a, b }) if *a == cursor => *b,
                Some(Entity::Line { a, b }) if *b == cursor => *a,
                _ => return invalid("Region loop contains disconnected entities"),
            };
        }
        if cursor != *start {
            return invalid("Region entity loop is open");
        }
        Ok(result)
    }
    fn closed_unbound_chains(&self) -> Vec<Vec<u64>> {
        let bound: BTreeSet<_> = self
            .profile_entities
            .values()
            .flatten()
            .copied()
            .chain(self.construction.iter().copied())
            .collect();
        let mut adjacent: BTreeMap<u64, Vec<(u64, u64)>> = BTreeMap::new();
        for (id, e) in &self.entities {
            if bound.contains(id) {
                continue;
            }
            if let Entity::Line { a, b } = e {
                adjacent.entry(*a).or_default().push((*id, *b));
                adjacent.entry(*b).or_default().push((*id, *a));
            }
        }
        let mut visited = BTreeSet::new();
        let mut loops = vec![];
        for (first, e) in &self.entities {
            let Entity::Line {
                a: start,
                b: second,
            } = e
            else {
                continue;
            };
            if bound.contains(first) || visited.contains(first) {
                continue;
            }
            let mut chain = vec![*first];
            let mut cursor = *second;
            let mut previous = *first;
            let mut closed = adjacent.get(start).is_some_and(|v| v.len() == 2);
            while cursor != *start {
                let Some(next) = adjacent.get(&cursor).filter(|v| v.len() == 2) else {
                    closed = false;
                    break;
                };
                let Some((edge, to)) = next.iter().find(|(id, _)| *id != previous) else {
                    closed = false;
                    break;
                };
                if chain.contains(edge) {
                    closed = false;
                    break;
                }
                chain.push(*edge);
                previous = *edge;
                cursor = *to;
            }
            visited.extend(chain.iter().copied());
            if closed && cursor == *start && chain.len() >= 3 {
                loops.push(chain);
            }
        }
        loops
    }
}
impl Sketch {
    pub fn graph_for_edit(&self) -> Result<SketchGraph> {
        if let Some(graph) = &self.graph {
            return Ok(graph.clone());
        }
        let mut graph = SketchGraph::default();
        for (id, p) in self.profile_ids.iter().zip(&self.profiles) {
            graph.add_profile(*id, p)?;
        }
        Ok(graph)
    }
    pub fn with_graph(&self, graph: SketchGraph, parameters: &Parameters) -> Result<Self> {
        let (mut graph, _) = graph.solve(parameters)?;
        let mut candidate = self.clone();
        for (id, p) in candidate.profile_ids.iter().zip(&mut candidate.profiles) {
            *p = graph.profile(*id, p)?;
            p.validate()?;
        }
        for entities in graph.closed_unbound_chains() {
            let profile = Profile::Polygon {
                points: graph.loop_points(&entities)?,
            };
            profile.validate()?;
            let id = candidate.next_profile_id;
            candidate.next_profile_id = id
                .checked_add(1)
                .ok_or_else(|| crate::Error::Invalid("Sketch profile IDs exhausted".into()))?;
            graph.profile_entities.insert(id, entities);
            candidate.profile_ids.push(id);
            candidate.profiles.push(profile);
        }
        candidate.graph = Some(graph);
        candidate.regions()?;
        Ok(candidate)
    }
    pub fn with_constraints(
        &self,
        constraints: Vec<Constraint>,
        parameters: &Parameters,
    ) -> Result<Self> {
        let mut graph = self.graph_for_edit()?;
        for c in constraints {
            graph.constrain(c)?;
        }
        self.with_graph(graph, parameters)
    }
    pub fn with_dragged_point(
        &self,
        point: u64,
        at: [f64; 2],
        parameters: &Parameters,
    ) -> Result<Self> {
        let mut graph = self.graph_for_edit()?;
        if !graph.points.contains_key(&point) {
            return invalid("Missing point to drag");
        }
        let temporary = graph.constrain(Constraint::Fixed { point, at })?;
        let (mut solved, _) = graph.solve(parameters)?;
        solved.constraints.remove(&temporary);
        self.with_graph(solved, parameters)
    }
    pub fn with_entity_dimension(
        &self,
        entity: u64,
        expression: String,
        parameters: &Parameters,
    ) -> Result<Self> {
        if crate::expression::length(&expression, parameters)? <= EPS {
            return invalid("Dimension must be positive");
        }
        let mut graph = self.graph_for_edit()?;
        let constraint = match graph.entities.get(&entity) {
            Some(Entity::Line { .. }) => Constraint::Length {
                line: entity,
                expression,
            },
            Some(Entity::Circle { .. }) => Constraint::Radius {
                circle: entity,
                expression,
            },
            _ => return invalid("Select a line or circle to dimension"),
        };
        graph.dimension(format!("entity.{entity}"), constraint)?;
        self.with_graph(graph, parameters)
    }
    pub fn with_construction(
        &self,
        entities: &[u64],
        construction: bool,
        parameters: &Parameters,
    ) -> Result<Self> {
        let mut graph = self.graph_for_edit()?;
        for id in entities {
            if !graph.entities.contains_key(id) {
                return invalid("Missing construction entity");
            }
            if construction {
                graph.construction.insert(*id);
            } else {
                graph.construction.remove(id);
            }
        }
        self.with_graph(graph, parameters)
    }
    pub fn without_constraint(&self, id: u64, parameters: &Parameters) -> Result<Self> {
        let mut graph = self.graph_for_edit()?;
        if graph.constraints.remove(&id).is_none() {
            return invalid("Missing constraint");
        }
        let keys: Vec<_> = graph
            .dimension_ids
            .iter()
            .filter_map(|(key, c)| (*c == id).then_some(key.clone()))
            .collect();
        graph.dimension_ids.retain(|_, c| *c != id);
        let mut candidate = self.clone();
        for key in keys {
            if let Some((profile, name)) = key.split_once('.') {
                if let Ok(profile) = profile.parse::<u64>() {
                    if let Some(map) = candidate.dimension_expressions.get_mut(&profile) {
                        map.remove(name);
                    }
                }
            }
        }
        candidate.with_graph(graph, parameters)
    }
    pub fn without_entities(&self, entities: &[u64], parameters: &Parameters) -> Result<Self> {
        let mut graph = self.graph_for_edit()?;
        let mut removed: BTreeSet<_> = entities.iter().copied().collect();
        if removed.iter().any(|id| !graph.entities.contains_key(id)) {
            return invalid("Missing entity to delete");
        }
        let profiles: BTreeSet<_> = graph
            .profile_entities
            .iter()
            .filter_map(|(id, edges)| edges.iter().any(|e| removed.contains(e)).then_some(*id))
            .collect();
        graph.entities.retain(|id, _| !removed.contains(id));
        graph.construction.retain(|id| !removed.contains(id));
        let used: BTreeSet<_> = graph
            .entities
            .values()
            .flat_map(|e| match e {
                Entity::Line { a, b } => vec![*a, *b],
                Entity::Circle { center, .. } => vec![*center],
            })
            .collect();
        removed.extend(graph.points.keys().filter(|id| !used.contains(id)).copied());
        graph.points.retain(|id, _| used.contains(id));
        let profile_dimensions: BTreeSet<_> = graph
            .dimension_ids
            .iter()
            .filter_map(|(key, id)| {
                key.split_once('.')
                    .and_then(|(p, _)| p.parse::<u64>().ok())
                    .filter(|p| profiles.contains(p))
                    .map(|_| *id)
            })
            .collect();
        graph.constraints.retain(|id, c| {
            !profile_dimensions.contains(id)
                && !c.references().iter().any(|id| removed.contains(id))
        });
        graph
            .dimension_ids
            .retain(|_, id| graph.constraints.contains_key(id));
        graph
            .profile_entities
            .retain(|id, _| !profiles.contains(id));
        let mut candidate = self.clone();
        let kept: Vec<_> = candidate
            .profile_ids
            .iter()
            .copied()
            .zip(candidate.profiles)
            .filter(|(id, _)| !profiles.contains(id))
            .collect();
        (candidate.profile_ids, candidate.profiles) = kept.into_iter().unzip();
        candidate
            .dimension_expressions
            .retain(|id, _| !profiles.contains(id));
        candidate.with_graph(graph, parameters)
    }
}
